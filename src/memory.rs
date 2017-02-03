use std::fmt;
use std::io::{Cursor, Seek, SeekFrom};
use std::str::FromStr;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use try_from::TryFrom;

use errors::*;
use op_code::OpCode;

#[derive(Debug)]
pub enum Target {
    Mem(Addr),
    Reg(Register),
}

impl PartialEq<Addr> for Target {
    fn eq(&self, addr: &Addr) -> bool {
        match *self {
            Target::Mem(ref a) => a == addr,
            _ => false,
        }
    }
}

impl PartialEq<Register> for Target {
    fn eq(&self, reg: &Register) -> bool {
        match *self {
            Target::Reg(ref r) => r == reg,
            _ => false,
        }
    }
}

impl PartialEq<Value> for Target {
    fn eq(&self, val: &Value) -> bool {
        match (self, val) {
            (&Target::Reg(ref tr), &Value::FromRegister(ref vr)) => tr == vr,
            _ => false,
        }
    }
}

impl FromStr for Target {
    type Err = Error;
    fn from_str(loc: &str) -> Result<Target> {
        if loc.starts_with("r") {
            Register::from_str(&loc[1..]).map(Target::Reg)
        } else {
            Addr::from_str(loc).map(Target::Mem)
        }
    }
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Target::Mem(addr) => write!(f, "{}", addr),
            Target::Reg(reg) => write!(f, "{}", reg),
        }
    }
}

/// Address as understood by the VM
#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub struct Addr(u16);

const MAX_ADDR: u16 = 32767;
pub const MAX_BYTES: usize = 65536;

impl FromStr for Addr {
    type Err = Error;
    fn from_str(s: &str) -> Result<Addr> {
        let u = if s.starts_with("0x") {
            u16::from_str_radix(&s[2..], 16).map_err(Error::from)?
        } else {
            u16::from_str(s).map_err(Error::from)?
        };
        if u > MAX_ADDR {
            bail!(ErrorKind::InvalidAddr(u as usize));
        }
        Ok(Addr(u))
    }
}

impl fmt::Display for Addr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{0:04x}", self.0)
    }
}

impl fmt::Debug for Addr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{0:04x}", self.0)
    }
}

impl From<u16> for Addr {
    fn from(u: u16) -> Addr {
        Addr(u)
    }
}

impl From<u64> for Addr {
    fn from(u: u64) -> Addr {
        Addr((u / 2) as _)
    }
}

impl From<Addr> for SeekFrom {
    fn from(a: Addr) -> SeekFrom {
        SeekFrom::Start((a.0 as u64) * 2)
    }
}

impl<'a> From<&'a Addr> for SeekFrom {
    fn from(a: &'a Addr) -> SeekFrom {
        SeekFrom::Start((a.0 as u64) * 2)
    }
}

impl From<Addr> for usize {
    fn from(a: Addr) -> usize {
        a.0 as usize
    }
}

impl From<Addr> for u16 {
    fn from(a: Addr) -> u16 {
        a.0 as u16
    }
}

impl<'a> From<&'a Addr> for usize {
    fn from(a: &'a Addr) -> usize {
        a.0 as usize
    }
}

pub struct AddrRange(Option<Addr>, Option<Addr>);

impl AddrRange {
    pub fn start(&self) -> usize {
        self.0.map(usize::from).unwrap_or(0)
    }
}

impl FromStr for AddrRange {
    type Err = Error;
    fn from_str(s: &str) -> Result<AddrRange> {
        let (s, e) = if s.contains("..") {
            let mut parts = s.splitn(2, "..");
            let s = match parts.next() {
                Some(s) if s.len() > 0 => Some(Addr::from_str(s)?),
                _ => None,
            };
            let e = match parts.next() {
                Some(e) if e.len() > 0 => Some(Addr::from_str(e)?),
                _ => None,
            };
            (s, e)
        } else {
            let s = Some(Addr::from_str(s)?);
            (s, s)
        };
        match (s, e) {
            (Some(ref sv), Some(ref ev)) if sv.0 > ev.0 => Ok(AddrRange(e, s)),
            _ => Ok(AddrRange(s, e)),
        }
    }
}

pub struct RegisterSet([u16; 8]);

pub struct RegisterSetIterator<'s> {
    register_set: &'s RegisterSet,
    index: usize,
}

impl<'s> Iterator for RegisterSetIterator<'s> {
    type Item = u16;
    fn next(&mut self) -> Option<u16> {
        if self.index < 8 {
            let result = self.register_set.0[self.index];
            self.index += 1;
            Some(result)
        } else {
            None
        }
    }
}

impl<'s> IntoIterator for &'s RegisterSet {
    type Item = u16;
    type IntoIter = RegisterSetIterator<'s>;

    fn into_iter(self) -> Self::IntoIter {
        RegisterSetIterator {
            register_set: self,
            index: 0,
        }
    }
}

impl RegisterSet {
    pub fn new() -> RegisterSet {
        RegisterSet([0; 8])
    }

    pub fn load(r: [u16; 8]) -> RegisterSet {
        RegisterSet(r)
    }

    pub fn read(&self, val: Value) -> u16 {
        match val {
            Value::Literal(u) => u,
            Value::FromRegister(ref reg) => self.0[usize::from(reg)],
        }
    }

    pub fn write_val(&mut self, reg: Register, val: Value) {
        let u = self.read(val);
        self.write_u16(reg, u);
    }

    pub fn write_u16(&mut self, reg: Register, u: u16) {
        self.0[usize::from(reg)] = u;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Register(usize);

impl fmt::Display for Register {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "r{}", self.0)
    }
}

impl TryFrom<u16> for Register {
    type Err = Error;
    fn try_from(u: u16) -> Result<Register> {
        match u {
            32768 => Ok(Register(0)),
            32769 => Ok(Register(1)),
            32770 => Ok(Register(2)),
            32771 => Ok(Register(3)),
            32772 => Ok(Register(4)),
            32773 => Ok(Register(5)),
            32774 => Ok(Register(6)),
            32775 => Ok(Register(7)),
            _ => bail!(ErrorKind::InvalidRegister(u)),
        }
    }
}
impl TryFrom<u8> for Register {
    type Err = Error;
    fn try_from(u: u8) -> Result<Register> {
        match u {
            0 => Ok(Register(0)),
            1 => Ok(Register(1)),
            2 => Ok(Register(2)),
            3 => Ok(Register(3)),
            4 => Ok(Register(4)),
            5 => Ok(Register(5)),
            6 => Ok(Register(6)),
            7 => Ok(Register(7)),
            _ => bail!(ErrorKind::InvalidRegister(u as u16)),
        }
    }
}
impl From<Register> for usize {
    fn from(r: Register) -> usize {
        r.0
    }
}
impl From<Register> for u8 {
    fn from(r: Register) -> u8 {
        r.0 as u8
    }
}
impl<'a> From<&'a Register> for usize {
    fn from(r: &'a Register) -> usize {
        r.0
    }
}
impl FromStr for Register {
    type Err = Error;
    fn from_str(n: &str) -> Result<Register> {
        let n = usize::from_str(n).map_err(Error::from)?;
        if n > 7 {
            bail!(ErrorKind::InvalidRegister(n as u16));
        }
        Ok(Register(n))
    }
}

#[derive(Clone, Copy)]
pub enum Value {
    Literal(u16),
    FromRegister(Register),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Value::Literal(l) => write!(f, "<{}, 0x{:04x}>", l, l),
            Value::FromRegister(r) => write!(f, "{}", r),
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Value::Literal(l) => write!(f, "<{}>", l),
            Value::FromRegister(r) => write!(f, "{}", r),
        }
    }
}

impl TryFrom<u16> for Value {
    type Err = Error;
    fn try_from(u: u16) -> Result<Value> {
        match u {
            0...MAX_ADDR => Ok(Value::Literal(u)),
            32768...32776 => Register::try_from(u).map(Value::FromRegister),
            _ => bail!(ErrorKind::InvalidValue(u)),
        }
    }
}

impl FromStr for Value {
    type Err = Error;
    fn from_str(v: &str) -> Result<Value> {
        let v = if v.starts_with("0x") {
            u16::from_str_radix(&v[2..], 16).map_err(Error::from)?
        } else if v.starts_with("b") {
            u16::from_str_radix(&v[1..], 2).map_err(Error::from)?
        } else {
            u16::from_str(v).map_err(Error::from)?
        };
        Value::try_from(v)
    }
}

pub struct Memory {
    ip: Cursor<Vec<u8>>,
    max_used_addr: Addr,
}

impl Memory {
    pub fn new(mut v: Vec<u8>) -> Result<Memory> {
        let byte_len = v.len();
        if MAX_BYTES < byte_len || byte_len % 2 != 0 {
            bail!(ErrorKind::InvalidMemorySize(byte_len));
        }
        let max_used_addr = Addr((byte_len as u16 / 2) - 1);
        v.resize(MAX_BYTES, 0);
        Ok(Memory {
            ip: Cursor::new(v),
            max_used_addr: max_used_addr,
        })
    }

    pub fn used_bytes(&self) -> u16 {
        (self.max_used_addr.0 + 1) * 2
    }

    pub fn set_ip(&mut self, addr: Addr) {
        let _ = self.ip.seek(addr.into());
    }

    pub fn ip(&self) -> Addr {
        self.ip.position().into()
    }

    pub fn read(&mut self, addr: Addr) -> Result<u16> {
        let ip = self.ip();
        self.set_ip(addr);
        let r = self.next_u16();
        self.set_ip(ip);
        r
    }

    pub fn write(&mut self, addr: Addr, val: u16) {
        let ip = self.ip();
        self.set_ip(addr);
        let _ = self.ip.write_u16::<LittleEndian>(val);
        if addr.0 > self.max_used_addr.0 {
            self.max_used_addr = addr;
        }
        self.set_ip(ip);
    }

    fn next_u16(&mut self) -> Result<u16> {
        self.ip.read_u16::<LittleEndian>().map_err(Error::from)
    }

    pub fn next_reg(&mut self) -> Result<Register> {
        self.next_u16().and_then(Register::try_from)
    }

    pub fn next_val(&mut self) -> Result<Value> {
        self.next_u16().and_then(Value::try_from)
    }

    pub fn next_instr(&mut self) -> Result<u16> {
        match self.next_val()? {
            Value::Literal(i) => Ok(i),
            Value::FromRegister(r) => bail!(ErrorKind::NonLiteralOpCode(r)),
        }
    }

    pub fn get_range(&self, r: &AddrRange) -> &[u8] {
        let s = r.0.map(|a| a.0).unwrap_or(0) as usize;
        let e = match r.1.map(|a| a.0) {
            Some(e) => e,
            None if s > self.max_used_addr.0 as usize => MAX_ADDR,
            None => self.max_used_addr.0,
        } as usize;
        // scale from u16 stride to u8
        let s = s * 2;
        let e = (e + 1) * 2;
        &self.ip.get_ref()[s..e]
    }

    pub fn fetch_op(&mut self) -> Result<OpCode> {
        let instr = self.next_instr()?;
        let op_code = match instr {
            0u16 => OpCode::Halt,
            1u16 => {
                OpCode::Set {
                    reg: self.next_reg()?,
                    val: self.next_val()?,
                }
            }
            2u16 => OpCode::Push { val: self.next_val()? },
            3u16 => OpCode::Pop { reg: self.next_reg()? },
            4u16 => {
                OpCode::Eq {
                    reg: self.next_reg()?,
                    val1: self.next_val()?,
                    val2: self.next_val()?,
                }
            }
            5u16 => {
                OpCode::Gt {
                    reg: self.next_reg()?,
                    val1: self.next_val()?,
                    val2: self.next_val()?,
                }
            }
            6u16 => OpCode::Jmp { addr: self.next_val()? },
            7u16 => {
                OpCode::Jt {
                    cond: self.next_val()?,
                    addr: self.next_val()?,
                }
            }
            8u16 => {
                OpCode::Jf {
                    cond: self.next_val()?,
                    addr: self.next_val()?,
                }
            }
            9u16 => {
                OpCode::Add {
                    reg: self.next_reg()?,
                    val1: self.next_val()?,
                    val2: self.next_val()?,
                }
            }
            10u16 => {
                OpCode::Mult {
                    reg: self.next_reg()?,
                    val1: self.next_val()?,
                    val2: self.next_val()?,
                }
            }
            11u16 => {
                OpCode::Mod {
                    reg: self.next_reg()?,
                    val1: self.next_val()?,
                    val2: self.next_val()?,
                }
            }
            12u16 => {
                OpCode::And {
                    reg: self.next_reg()?,
                    val1: self.next_val()?,
                    val2: self.next_val()?,
                }
            }
            13u16 => {
                OpCode::Or {
                    reg: self.next_reg()?,
                    val1: self.next_val()?,
                    val2: self.next_val()?,
                }
            }
            14u16 => {
                OpCode::Not {
                    reg: self.next_reg()?,
                    val: self.next_val()?,
                }
            }
            15u16 => {
                OpCode::Rmem {
                    reg: self.next_reg()?,
                    addr: self.next_val()?,
                }
            }
            16u16 => {
                OpCode::Wmem {
                    addr: self.next_val()?,
                    val: self.next_val()?,
                }
            }
            17u16 => OpCode::Call { addr: self.next_val()? },
            18u16 => OpCode::Ret,
            19u16 => OpCode::Out { c: self.next_val()? },
            20u16 => OpCode::In { reg: self.next_reg()? },
            21u16 => OpCode::Noop,
            u => bail!(ErrorKind::InvalidOpCode(u)),
        };
        Ok(op_code)
    }
}
