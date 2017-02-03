mod breakpoint;

use std;
use std::ascii::AsciiExt;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::str::FromStr;

use byteorder::WriteBytesExt;
use try_from::TryFrom;

use errors::*;
use machine::*;
use memory;
use op_code;

enum VmState {
    Stalled(StalledMachine),
    Running(Machine),
    Halted(HaltedMachine),
}

impl AsMut<Inspectable + 'static> for VmState {
    fn as_mut(&mut self) -> &mut (Inspectable + 'static) {
        match self {
            &mut VmState::Running(ref mut r) => r,
            &mut VmState::Stalled(ref mut s) => s,
            &mut VmState::Halted(ref mut h) => h,
        }
    }
}

impl AsRef<Inspectable + 'static> for VmState {
    fn as_ref(&self) -> &(Inspectable + 'static) {
        match self {
            &VmState::Running(ref r) => r,
            &VmState::Stalled(ref s) => s,
            &VmState::Halted(ref h) => h,
        }
    }
}

enum Sink {
    StdOut,
    File(File),
}

pub struct Debugger {
    state: VmState,
    breakpoints: Vec<breakpoint::Breakpoint>,
    output: Option<Sink>,
}

impl Debugger {
    fn get_input() -> Result<Option<String>> {
        print!("vm requesting input (! to break to debugger): ");
        let _ = std::io::stdout().flush();
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        Ok(if "!" == input.trim() {
            None
        } else {
            Some(input)
        })
    }

    fn step_vm(mut self) -> Result<Debugger> {
        if self.output.is_some() {
            let i = self.curr_instr()?;
            match self.output {
                Some(Sink::StdOut) => println!("{}", i),
                Some(Sink::File(ref mut f)) => writeln!(f, "{}", i)?,
                None => unreachable!(),
            }
        }
        let Debugger { mut state, breakpoints, output } = self;
        let machine = match state {
            VmState::Running(m) => m,
            VmState::Stalled(stalled) => {
                if let Some(input) = Debugger::get_input()? {
                    stalled.set_input(input)?
                } else {
                    return Ok(Debugger {
                        state: VmState::Stalled(stalled),
                        breakpoints: breakpoints,
                        output: output,
                    });
                }
            }
            VmState::Halted(_) => {
                println!("cannot step Halted VM");
                return Ok(Debugger {
                    state: state,
                    breakpoints: breakpoints,
                    output: output,
                });
            }
        };
        state = match machine.step()? {
            OpResult::Continue(m) => VmState::Running(m),
            OpResult::Output(c, m) => {
                print!("{}", c);
                VmState::Running(m)
            }
            OpResult::Input(stalled) => {
                if let Some(input) = Debugger::get_input()? {
                    VmState::Running(stalled.set_input(input)?)
                } else {
                    VmState::Stalled(stalled)
                }
            }
            OpResult::Halted(halted) => VmState::Halted(halted),
        };

        Ok(Debugger {
            state: state,
            breakpoints: breakpoints,
            output: output,
        })
    }

    fn triggered_breakpoint(&mut self) -> Result<Option<breakpoint::Reason>> {
        Ok(match &mut self.state {
            &mut VmState::Running(ref mut m) => {
                let (op, decoded_op) = m.peek_instr()?;
                if let Some(ip) = m.ip() {
                    self.breakpoints
                        .iter()
                        .find(|bp| bp.is_triggered(&ip, &op, &decoded_op))
                        .map(breakpoint::Reason::Triggered)
                } else {
                    None
                }
            }
            &mut VmState::Stalled(_) => Some(breakpoint::Reason::Stalled),
            &mut VmState::Halted(_) => Some(breakpoint::Reason::Halted),
        })
    }

    fn prompt(&self) {
        if let Some(ip) = self.state.as_ref().ip() {
            print!("\n{:?} > ", ip);
        } else {
            print!("\nHALTED > ");
        }
        let _ = std::io::stdout().flush();
    }

    fn scan_strings(&self) -> Result<()> {
        let mem = self.state.as_ref().memory();
        let mut cur_string = String::new();
        let mem: Vec<u8> = memory::AddrRange::try_from("..")
            .map(|r| mem.get_range(&r))?
            .iter()
            .map(|v| *v)
            .collect();
        let mut mem = memory::Memory::new(mem)?;
        let regs = self.state.as_ref().registers();
        while let Ok(op) = mem.fetch_op() {
            match op {
                op_code::OpCode::Out { c } => {
                    let c = regs.read(c) as u8 as char;
                    if '\n' == c {
                        println!("{}", cur_string);
                        cur_string.clear();
                    } else {
                        cur_string.push(c);
                    }
                }
                _ => {}
            }
        }
        if !cur_string.is_empty() {
            println!("{}", cur_string);
        }
        Ok(())
    }

    fn add_breakpoint(&mut self, op: &str, loc: &str) -> Result<()> {
        let bp = match op {
            "r" => breakpoint::Breakpoint::read(loc),
            "w" => breakpoint::Breakpoint::write(loc),
            "a" => breakpoint::Breakpoint::access(loc),
            "@" => breakpoint::Breakpoint::at(loc),
            o => bail!("unknown breakpoint op {}", o),

        }?;
        self.breakpoints.push(bp);
        Ok(())
    }

    fn list_breakpoints(&self) {
        for i in 0..self.breakpoints.len() {
            println!("{}: {}", i, self.breakpoints[i])
        }
    }

    fn delete_breakpoint(&mut self, n: &str) -> Result<()> {
        let n = n.trim();
        if "*" == n {
            self.breakpoints.clear();
        } else {
            let n = usize::try_from(n)?;
            if n >= self.breakpoints.len() {
                bail!("no such breakpoint {}", n);
            } else {
                self.breakpoints.remove(n);
            }
        }
        Ok(())
    }

    fn examine_mem(&self, addrs: &str) -> Result<()> {
        const WIDTH: usize = 16;
        let mem = self.state.as_ref().memory();
        let range = memory::AddrRange::try_from(addrs)?;
        let mem = mem.get_range(&range).chunks(WIDTH);
        let mut s = range.start();
        for row in mem {
            print!("{:04x}: ", s);
            for x in row {
                print!("{:02x} ", x);
            }
            for _ in row.len()..WIDTH {
                print!("   ");
            }
            for c in row {
                let c = *c as char;
                if c.is_ascii() && !c.is_control() {
                    print!("{}", c);
                } else {
                    print!(".");
                }
            }
            println!();
            s += WIDTH;
        }
        Ok(())
    }

    fn curr_instr(&mut self) -> Result<String> {
        let regs = self.state
            .as_ref()
            .registers()
            .into_iter()
            .map(|r| format!("{}", r))
            .collect::<Vec<_>>();
        let m = self.state.as_mut();
        if let Some(ip) = m.ip() {
            m.peek_instr().map(|(r, d)| match r {
                op_code::OpCode::Call { addr } => {
                    let regs = regs.join(",");
                    format!("{:?}: {} | {} <- {}({})", ip, r, d, addr, regs)
                }
                op_code::OpCode::Ret { .. } => format!("{:?}: {} | {} -> {}", ip, r, d, regs[0]),
                _ => format!("{:?}: {} | {}", ip, r, d),
            })
        } else {
            Ok(format!("MACHINE HALTED"))
        }
    }
    fn set_output(&mut self, sink: Option<&str>) -> Result<()> {
        self.output = match sink {
            Some("-") => {
                println!("logging instructions to stdout");
                Some(Sink::StdOut)
            }
            Some(f) => {
                println!("logging instructions to {}", f);
                Some(Sink::File(File::create(f)?))
            }
            None => {
                println!("instruction logging disabled");
                None
            }
        };
        Ok(())
    }

    fn dump_mem(&self, mut file: File) -> Result<()> {
        let mem = self.state.as_ref().memory();
        memory::AddrRange::try_from("..")
            .map(|r| mem.get_range(&r))
            .and_then(|mem| file.write_all(mem).map_err(Error::from))
    }

    fn save_vm(&self, mut file: File) -> Result<()> {
        let (vm_state, reg) = match self.state {
            VmState::Stalled(ref m) => (0, m.reg_u8()),
            VmState::Running(_) => (1, 0),
            VmState::Halted(_) => (2, 0),
        };
        debug!("vm_state: {}, reg: {}", vm_state, reg);
        file.write_u8(vm_state)?;
        file.write_u8(reg)?;
        self.state
            .as_ref()
            .as_bytes()
            .and_then(|bytes| file.write_all(&bytes[..]).map_err(Error::from))
    }

    fn load_vm(&mut self, file: File) -> Result<()> {
        let mut save_data = std::io::BufReader::new(file);
        let mut data = Vec::with_capacity(memory::MAX_BYTES);
        let bytes_read = save_data.read_to_end(&mut data)?;
        debug!("read {} save file bytes", bytes_read);
        debug!("vm_state: {}, reg: {}", data[0], data[1]);
        let machine = Machine::try_from(&data[2..])?;
        self.state = match data[0] {
            0 => {
                VmState::Stalled(StalledMachine::new(machine, memory::Register::try_from(data[1])?))
            }
            1 => VmState::Running(machine),
            2 => VmState::Halted(HaltedMachine::new(machine)),
            v => bail!("unknown VmState {}", v),
        };
        Ok(())
    }

    fn write_reg(&mut self, n: &str, v: &str) -> Result<()> {
        let r = memory::Register::from_str(n)?;
        let v = memory::Value::from_str(v)?;
        self.state.as_mut().write_reg(r, v);
        Ok(())
    }

    fn show_stack(&self, n: Option<&str>) -> Result<()> {
        let stack = self.state.as_ref().stack();
        let stack_len = stack.len();
        let n = match n {
            Some(n) => {
                let n = usize::from_str(n)?;
                if n > stack_len { stack_len } else { n }
            }
            None => stack_len,
        };
        for (i, v) in stack.iter().rev().take(n).enumerate() {
            let v = memory::Value::try_from(*v)?;
            println!("{:04}: {} {:?}", i, v, v);
        }
        Ok(())
    }

    fn show_registers(&self, r: &str) -> Result<()> {
        let mut regs = self.state.as_ref().registers().into_iter();
        if "r" == r {
            let mut i = 0;
            for r in regs {
                print!("r{}: 0x{:04x} {} {:?}\n",
                       i,
                       r,
                       r,
                       memory::Value::try_from(r));
                i += 1;
            }
        } else {
            let i = usize::from(memory::Register::from_str(&r[1..])?);
            if let Some(r) = regs.nth(i) {
                print!("r{}: 0x{:04x} {} {:?}\n",
                       i,
                       r,
                       r,
                       memory::Value::try_from(r));
            }
        }
        Ok(())
    }
}

pub fn debug<P: AsRef<Path>>(rom_path: P) -> Result<()> {
    let mut input = String::new();
    let mut debugger = Debugger {
        state: VmState::Running(Machine::new(rom_path)?),
        breakpoints: Vec::new(),
        output: None,
    };
    loop {
        debugger.prompt();
        input.clear();
        std::io::stdin().read_line(&mut input)?;
        let mut parts = input.split_whitespace();
        if let Some(cmd) = parts.next() {
            match cmd {
                "c" => {
                    loop {
                        debugger = debugger.step_vm()?;
                        match debugger.triggered_breakpoint() {
                            Ok(Some(r)) => {
                                println!("breaking: {}", r);
                                break;
                            }
                            Err(e) => println!("error testing breakpoint: {}", e),
                            _ => {}
                        }
                    }
                }
                "s" => {
                    let steps = if let Some(steps) = parts.next() {
                        if let Ok(steps) = u64::from_str(steps) {
                            steps
                        } else {
                            println!("invalid step count: {}", steps);
                            continue;
                        }
                    } else {
                        1
                    };

                    for _ in 0..steps {
                        debugger = debugger.step_vm()?;
                        match debugger.triggered_breakpoint() {
                            Ok(Some(r)) => {
                                println!("breaking: {}", r);
                                break;
                            }
                            Err(e) => println!("error testing breakpoint: {}", e),
                            _ => {}
                        }
                    }
                }
                "i" => {
                    match debugger.curr_instr() {
                        Ok(i) => println!("{}", i),
                        Err(e) => println!("error displaying current instruction: {} ", e),
                    }
                }
                "x" => {
                    if let Some(loc) = parts.next() {
                        let r = if "s" == loc {
                            debugger.show_stack(parts.next())
                        } else if loc.starts_with("r") {
                            debugger.show_registers(loc)
                        } else {
                            debugger.examine_mem(loc)
                        };
                        if let Err(e) = r {
                            println!("error examining memory: {}", e);
                        }
                    } else {
                        println!("must specify location to examine");
                    }
                }
                "d" => {
                    if let Some(file) = parts.next() {
                        if let Err(e) = File::create(file)
                            .map_err(Error::from)
                            .and_then(|f| debugger.dump_mem(f)) {
                            println!("unable to create memory dump file: {}", e);
                        }
                    } else {
                        println!("must specify output file");
                    }
                }
                "f" => {
                    if let Err(e) = debugger.scan_strings() {
                        println!("unable to scan memory for strings: {}", e)
                    }
                }
                "b" => {
                    match (parts.next(), parts.next()) {
                        (Some(o), Some(l)) => {
                            if let Err(e) = debugger.add_breakpoint(o, l) {
                                println!("error adding breakpoint: {}", e);
                            }
                        }
                        _ => println!("must specify op and loc"),
                    }
                }
                "bl" => debugger.list_breakpoints(),
                "bx" => {
                    if let Some(n) = parts.next() {
                        if let Err(e) = debugger.delete_breakpoint(n) {
                            println!("error deleting breakpoint: {}", e);
                        }
                    } else {
                        println!("must specify breakpoint to delete (\"*\" for all)");
                    }
                }
                "v" => {
                    if let Some(file) = parts.next() {
                        if let Err(e) = File::create(file)
                            .map_err(Error::from)
                            .and_then(|f| debugger.save_vm(f)) {
                            println!("unable to create VM save file: {}", e);
                        }
                    } else {
                        println!("must specify output file");
                    }
                }
                "w" => {
                    match (parts.next(), parts.next()) {
                        (Some(n), Some(v)) => {
                            if let Err(e) = debugger.write_reg(n, v) {
                                println!("could not write register: {}", e);
                            }
                        }
                        _ => println!("must specify register and value"),
                    }
                }
                ">" => {
                    if let Err(e) = debugger.set_output(parts.next()) {
                        println!("error setting instruction logging: {}", e);
                    }
                }
                "l" => {
                    if let Some(file) = parts.next() {
                        if let Err(e) = File::open(file)
                            .map_err(Error::from)
                            .and_then(|f| debugger.load_vm(f)) {
                            println!("unable to load VM save file: {}", e);
                        }
                    } else {
                        println!("must specify input file");
                    }
                }
                "q" => {
                    println!("quitting...");
                    break;
                }
                "h" => {
                    println!(r#"
v file  - save vm state to <file>
l file  - load vm state from <file>
> [<file|->]
        - log instructions to <file>. if '-' is specified, instructions will be printed to STDOUT
          logging is turned off if no argument specified.
c       - continue execution
i       - show current instruction
s [n]   - step execution n times (once if unspecified)
w n val - write val (0..32767) to register n
x addr[..addr]
        - examine memory contents at addr. a range can be specified, e.g. 0x000f..0x00f0
         when specifying a range, omitting the first, second, or both addresses will
         extend the range from the start or to the end
x s [n] - show stack contents. If <n> is specified, at most <n> entries will be shown (top down)
x r[0-7]
        - show register contents ('r' shows all registers)
d file  - dump the memory contents to file
f       - scan memory for strings and output them
b op loc
        - add a conditional breakpoint
          op: one of:
            @ (at)     - break when instruction pointer hits given address
            r (read)   - break when an instruction reads from given address or register
            w (write)  - break when an instruction writes to given address or register
            a (access) - break when an instruction reads or writes given address or register
          loc: location to watch, either one of r[0...7] for registers,
               or 0x<addr> for memory location.
               NB: @ op requires a memory address
bl      - list breakpoints
bx n    - delete breakpoint n ("*" for all breakpoints)
q       - quit
"#);
                }
                c => println!("unrecognized command '{}', try 'h' for help", c),
            }
        }
    }
    Ok(())
}
