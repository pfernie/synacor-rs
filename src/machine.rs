use std;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::str::FromStr;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use try_from::TryFrom;

use errors::*;
use memory::*;
use op_code::{OpCode, DecodedOpCode};

pub trait Inspectable {
    fn ip(&self) -> Option<Addr>;
    fn registers(&self) -> &RegisterSet;
    fn memory(&self) -> &Memory;
    fn stack(&self) -> &[u16];
    fn as_bytes(&self) -> Result<Vec<u8>>;
    fn write_reg(&mut self, Register, Value);
    fn peek_instr(&mut self) -> Result<(OpCode, DecodedOpCode)>;
}

pub struct Machine {
    memory: Memory,
    registers: RegisterSet,
    stack: Vec<u16>,
    input_buffer: String,
}

impl<'a> TryFrom<&'a [u8]> for Machine {
    type Err = Error;
    fn try_from(d: &'a [u8]) -> Result<Machine> {
        let mut c = std::io::Cursor::new(d);
        let ip: Addr = c.read_u16::<LittleEndian>()?.into();
        debug!("ip: {:?}", ip);
        let mem_bytes = c.read_u16::<LittleEndian>()? as usize;
        debug!("mem_bytes: {:?}", mem_bytes);
        let mut mem = Vec::with_capacity(mem_bytes);
        mem.resize(mem_bytes, 0);
        let mut memory = {
            let v = c.get_ref();
            mem.copy_from_slice(&v[4..(4 + mem_bytes)]);
            Memory::new(mem)?
        };
        memory.set_ip(ip.into());
        c.seek(SeekFrom::Current(mem_bytes as i64 / 2))?;
        let registers = {
            let mut r = [0u16; 8];
            for i in 0..8 {
                r[i] = c.read_u16::<LittleEndian>()?;
            }
            RegisterSet::load(r)
        };
        debug!("registers:");
        for r in &registers {
            debug!("0x{0:04x}", r);
        }
        let stack_bytes = c.read_u16::<LittleEndian>()? as usize;
        debug!("stack_bytes: {}", stack_bytes);
        let stack_len = stack_bytes / 2;
        debug!("stack_len: {}", stack_len);
        let mut stack = Vec::with_capacity(stack_len);
        for i in 0..stack_len {
            let v = c.read_u16::<LittleEndian>()?;
            debug!("{0:03}: 0x{1:04x}", i, v);
            stack.push(v);
        }
        Ok(Machine {
            memory: memory,
            registers: registers,
            stack: stack,
            input_buffer: String::new(),
        })
    }
}

impl Machine {
    pub fn new<P: AsRef<Path>>(rom_path: P) -> Result<Machine> {
        let rom_file = std::fs::File::open(rom_path)?;
        let mut rom_data = std::io::BufReader::new(rom_file);
        let memory = {
            let mut memory = Vec::with_capacity(::memory::MAX_BYTES);
            let bytes_read = rom_data.read_to_end(&mut memory)?;
            debug!("read {} ROM bytes", bytes_read);
            Memory::new(memory)?
        };
        Ok(Machine {
            memory: memory,
            registers: RegisterSet::new(),
            stack: Vec::new(),
            input_buffer: String::new(),
        })
    }

    pub fn step(mut self) -> Result<OpResult> {
        let op_code = self.memory.fetch_op()?;
        match op_code.decode(&self.registers, self.stack.last().map(|h| *h))? {
            DecodedOpCode::Halt => return Ok(OpResult::Halted(HaltedMachine(self))),
            DecodedOpCode::Out { c } => {
                return Ok(OpResult::Output(c, self));
            }
            DecodedOpCode::Noop => {}
            DecodedOpCode::Jmp { addr } => {
                self.memory.set_ip(addr);
            }
            DecodedOpCode::Jt { cond, addr } => {
                if cond != 0 {
                    self.memory.set_ip(addr);
                }
            }
            DecodedOpCode::Jf { cond, addr } => {
                if cond == 0 {
                    self.memory.set_ip(addr);
                }
            }
            DecodedOpCode::Set { reg, val } => {
                self.registers.write_u16(reg, val);
            }
            DecodedOpCode::Add { reg, val1, val2 } => {
                self.registers.write_u16(reg, (val1 + val2) % 32768);
            }
            DecodedOpCode::Mult { reg, val1, val2 } => {
                self.registers.write_u16(reg, (((val1 as u64) * (val2 as u64)) % 32768) as _);
            }
            DecodedOpCode::Mod { reg, val1, val2 } => {
                self.registers.write_u16(reg, (val1 % val2) % 32768);
            }
            DecodedOpCode::Eq { reg, val1, val2 } => {
                self.registers.write_u16(reg, if val1 == val2 { 1 } else { 0 });
            }
            DecodedOpCode::Push { val } => {
                self.stack.push(val);
            }
            DecodedOpCode::Pop { reg } => {
                if let Some(v) = self.stack.pop() {
                    self.registers.write_u16(reg, v);
                } else {
                    bail!(ErrorKind::EmptyStack);
                }
            }
            DecodedOpCode::Gt { reg, val1, val2 } => {
                self.registers.write_u16(reg, if val1 > val2 { 1 } else { 0 });
            }
            DecodedOpCode::And { reg, val1, val2 } => {
                self.registers.write_u16(reg, val1 & val2);
            }
            DecodedOpCode::Or { reg, val1, val2 } => {
                self.registers.write_u16(reg, val1 | val2);
            }
            DecodedOpCode::Not { reg, val } => {
                self.registers.write_u16(reg, !val & 0b111111111111111);
            }
            DecodedOpCode::Call { addr } => {
                self.stack.push(usize::from(self.memory.ip()) as u16);
                self.memory.set_ip(addr);
            }
            DecodedOpCode::Rmem { reg, addr } => {
                let v = self.memory.read(addr).and_then(Value::try_from)?;
                self.registers.write_val(reg, v);
            }
            DecodedOpCode::Wmem { addr, val } => {
                self.memory.write(addr, val);
            }
            DecodedOpCode::Ret { addr } => {
                if let Some(a) = addr {
                    self.stack.pop();
                    self.memory.set_ip(a);
                } else {
                    return Ok(OpResult::Halted(HaltedMachine(self)));
                }
            }
            DecodedOpCode::In { reg } => {
                if 0 == self.input_buffer.len() {
                    return Ok(OpResult::Input(StalledMachine(self, reg)));
                } else {
                    self.write_reg_from_buffer(reg)?;
                }
            }
        }
        Ok(OpResult::Continue(self))
    }

    fn write_reg_from_buffer(&mut self, reg: Register) -> Result<()> {
        let v = Value::try_from(self.input_buffer.remove(0) as u16)?;
        Ok(self.registers.write_val(reg, v))
    }
}

impl Inspectable for Machine {
    fn ip(&self) -> Option<Addr> {
        Some(self.memory.ip())
    }

    fn registers(&self) -> &RegisterSet {
        &self.registers
    }

    fn memory(&self) -> &Memory {
        &self.memory
    }

    fn stack(&self) -> &[u16] {
        &self.stack
    }

    fn as_bytes(&self) -> Result<Vec<u8>> {
        let mem_bytes = self.memory.used_bytes() as usize;
        let stack_len = self.stack.len();
        let stack_bytes = stack_len * 2;
        let tot_bytes = 2 /* ip */
            + 2 /* mem_bytes */ + mem_bytes
            + 16 /* registers */
            + 2 /* stack_bytes */ + stack_bytes;
        let mut buf = Vec::with_capacity(tot_bytes);
        buf.resize(tot_bytes, 0);
        let mut c = std::io::Cursor::new(buf);
        debug!("ip: {:?}", self.memory.ip());
        c.write_u16::<LittleEndian>(self.memory.ip().into())?;
        debug!("used_bytes: {}", mem_bytes);
        c.write_u16::<LittleEndian>(mem_bytes as u16)?;
        {
            let src = self.memory.get_range(&AddrRange::from_str("..")?);
            let dst = &mut c.get_mut()[4..(4 + mem_bytes)];
            dst.copy_from_slice(src);
        }
        c.seek(SeekFrom::Current(mem_bytes as i64 / 2))?;
        debug!("registers:");
        for r in &self.registers {
            debug!("0x{0:04x}", r);
            c.write_u16::<LittleEndian>(r)?;
        }
        debug!("stack_len: {}", stack_len);
        debug!("stack_bytes: {}", stack_bytes);
        c.write_u16::<LittleEndian>(stack_bytes as u16)?;
        debug!("stack:");
        for i in 0..self.stack.len() {
            let v = self.stack[i];
            debug!("{0:03}: 0x{1:04x}", i, v);
            c.write_u16::<LittleEndian>(v)?;
        }
        Ok(c.into_inner())
    }

    fn write_reg(&mut self, reg: Register, val: Value) {
        self.registers.write_val(reg, val);
    }

    fn peek_instr(&mut self) -> Result<(OpCode, DecodedOpCode)> {
        let ip = self.memory.ip();
        let op_code = self.memory.fetch_op()?;
        let decoded = op_code.decode(&self.registers, self.stack.last().map(|h| *h))?;
        self.memory.set_ip(ip);
        Ok((op_code, decoded))
    }
}

pub struct StalledMachine(Machine, Register);
impl StalledMachine {
    pub fn new(m: Machine, r: Register) -> StalledMachine {
        StalledMachine(m, r)
    }

    pub fn reg_u8(&self) -> u8 {
        self.1.into()
    }

    pub fn set_input(mut self, input: String) -> Result<Machine> {
        self.0.input_buffer = input;
        self.0.write_reg_from_buffer(self.1)?;
        Ok(self.0)
    }
}

impl Inspectable for StalledMachine {
    fn ip(&self) -> Option<Addr> {
        Some(self.0.memory.ip())
    }

    fn registers(&self) -> &RegisterSet {
        &self.0.registers
    }

    fn memory(&self) -> &Memory {
        &self.0.memory
    }

    fn stack(&self) -> &[u16] {
        &self.0.stack
    }

    fn as_bytes(&self) -> Result<Vec<u8>> {
        self.0.as_bytes()
    }

    fn write_reg(&mut self, reg: Register, val: Value) {
        self.0.registers.write_val(reg, val);
    }

    fn peek_instr(&mut self) -> Result<(OpCode, DecodedOpCode)> {
        self.0.peek_instr()
    }
}

pub struct HaltedMachine(Machine);

impl HaltedMachine {
    pub fn new(m: Machine) -> HaltedMachine {
        HaltedMachine(m)
    }
}

impl Inspectable for HaltedMachine {
    fn ip(&self) -> Option<Addr> {
        None
    }

    fn registers(&self) -> &RegisterSet {
        &self.0.registers
    }

    fn memory(&self) -> &Memory {
        &self.0.memory
    }

    fn stack(&self) -> &[u16] {
        &self.0.stack
    }

    fn as_bytes(&self) -> Result<Vec<u8>> {
        self.0.as_bytes()
    }

    fn write_reg(&mut self, reg: Register, val: Value) {
        self.0.registers.write_val(reg, val);
    }

    fn peek_instr(&mut self) -> Result<(OpCode, DecodedOpCode)> {
        self.0.peek_instr()
    }
}

pub enum OpResult {
    Input(StalledMachine),
    Output(char, Machine),
    Continue(Machine),
    Halted(HaltedMachine),
}
