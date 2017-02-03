use std::{fmt, result};

use errors::*;
use memory::{Addr, Register, RegisterSet, Target, Value};

pub trait OpAccess {
    fn reads(&self, tgt: &Target) -> bool;
    fn writes(&self, tgt: &Target) -> bool;
    fn accesses(&self, tgt: &Target) -> bool {
        self.reads(tgt) || self.writes(tgt)
    }
}

#[derive(Debug)]
pub enum OpCode {
    Halt,
    Set { reg: Register, val: Value },
    Push { val: Value },
    Pop { reg: Register },
    Eq {
        reg: Register,
        val1: Value,
        val2: Value,
    },
    Gt {
        reg: Register,
        val1: Value,
        val2: Value,
    },
    Jmp { addr: Value },
    Jt { cond: Value, addr: Value },
    Jf { cond: Value, addr: Value },
    Add {
        reg: Register,
        val1: Value,
        val2: Value,
    },
    Mult {
        reg: Register,
        val1: Value,
        val2: Value,
    },
    Mod {
        reg: Register,
        val1: Value,
        val2: Value,
    },
    And {
        reg: Register,
        val1: Value,
        val2: Value,
    },
    Or {
        reg: Register,
        val1: Value,
        val2: Value,
    },
    Not { reg: Register, val: Value },
    Rmem { reg: Register, addr: Value },
    Wmem { addr: Value, val: Value },
    Call { addr: Value },
    Ret,
    Out { c: Value },
    In { reg: Register },
    Noop,
}

impl fmt::Display for OpCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        match *self {
            OpCode::Halt => write!(f, "halt"),
            OpCode::Set { reg, val } => write!(f, "set {} {}", reg, val),
            OpCode::Push { val } => write!(f, "push {}", val),
            OpCode::Pop { reg } => write!(f, "pop {}", reg),
            OpCode::Eq { reg, val1, val2 } => write!(f, "eq {} {} {}", reg, val1, val2),
            OpCode::Gt { reg, val1, val2 } => write!(f, "gt {} {} {}", reg, val1, val2),
            OpCode::Jmp { addr } => write!(f, "jmp {}", addr),
            OpCode::Jt { cond, addr } => write!(f, "jt {} {}", cond, addr),
            OpCode::Jf { cond, addr } => write!(f, "jf {} {}", cond, addr),
            OpCode::Add { reg, val1, val2 } => write!(f, "add {} {} {}", reg, val1, val2),
            OpCode::Mult { reg, val1, val2 } => write!(f, "mult {} {} {}", reg, val1, val2),
            OpCode::Mod { reg, val1, val2 } => write!(f, "mod {} {} {}", reg, val1, val2),
            OpCode::And { reg, val1, val2 } => write!(f, "and {} {} {}", reg, val1, val2),
            OpCode::Or { reg, val1, val2 } => write!(f, "or {} {} {}", reg, val1, val2),
            OpCode::Not { reg, val } => write!(f, "not {} {}", reg, val),
            OpCode::Rmem { reg, addr } => write!(f, "rmem {} {}", reg, addr),
            OpCode::Wmem { addr, val } => write!(f, "wmem {} {}", addr, val),
            OpCode::Call { addr } => write!(f, "call {}", addr),
            OpCode::Ret => write!(f, "ret"),
            OpCode::Out { c } => write!(f, "out {}", c),
            OpCode::In { reg } => write!(f, "in {}", reg),
            OpCode::Noop => write!(f, "noop"),
        }
    }
}

impl OpAccess for OpCode {
    fn reads(&self, tgt: &Target) -> bool {
        match *self {
            OpCode::Halt => false,
            OpCode::Set { ref val, .. } => tgt == val,
            OpCode::Push { ref val } => tgt == val,
            OpCode::Pop { .. } => false,
            OpCode::Eq { ref val1, ref val2, .. } => tgt == val1 || tgt == val2,
            OpCode::Gt { ref val1, ref val2, .. } => tgt == val1 || tgt == val2,
            OpCode::Jmp { ref addr } => tgt == addr,
            OpCode::Jt { ref cond, ref addr } => tgt == cond || tgt == addr,
            OpCode::Jf { ref cond, ref addr } => tgt == cond || tgt == addr,
            OpCode::Add { ref val1, ref val2, .. } => tgt == val1 || tgt == val2,
            OpCode::Mult { ref val1, ref val2, .. } => tgt == val1 || tgt == val2,
            OpCode::Mod { ref val1, ref val2, .. } => tgt == val1 || tgt == val2,
            OpCode::And { ref val1, ref val2, .. } => tgt == val1 || tgt == val2,
            OpCode::Or { ref val1, ref val2, .. } => tgt == val1 || tgt == val2,
            OpCode::Not { ref val, .. } => tgt == val,
            OpCode::Rmem { ref addr, .. } => tgt == addr,
            OpCode::Wmem { ref val, .. } => tgt == val,
            OpCode::Call { ref addr } => tgt == addr,
            OpCode::Ret => false,
            OpCode::Out { ref c } => tgt == c,
            OpCode::In { .. } => false,
            OpCode::Noop => false,
        }
    }

    fn writes(&self, tgt: &Target) -> bool {
        match *self {
            OpCode::Halt => false,
            OpCode::Set { ref reg, .. } => tgt == reg,
            OpCode::Push { .. } => false,
            OpCode::Pop { ref reg } => tgt == reg,
            OpCode::Eq { ref reg, .. } => tgt == reg,
            OpCode::Gt { ref reg, .. } => tgt == reg,
            OpCode::Jmp { .. } => false,
            OpCode::Jt { .. } => false,
            OpCode::Jf { .. } => false,
            OpCode::Add { ref reg, .. } => tgt == reg,
            OpCode::Mult { ref reg, .. } => tgt == reg,
            OpCode::Mod { ref reg, .. } => tgt == reg,
            OpCode::And { ref reg, .. } => tgt == reg,
            OpCode::Or { ref reg, .. } => tgt == reg,
            OpCode::Not { ref reg, .. } => tgt == reg,
            OpCode::Rmem { ref reg, .. } => tgt == reg,
            OpCode::Wmem { ref addr, .. } => tgt == addr,
            OpCode::Call { .. } => false,
            OpCode::Ret => false,
            OpCode::Out { .. } => false,
            OpCode::In { ref reg } => tgt == reg,
            OpCode::Noop => false,
        }
    }
}

impl OpCode {
    pub fn decode(&self, registers: &RegisterSet, ret: Option<u16>) -> Result<DecodedOpCode> {
        Ok(match *self {
            OpCode::Halt => DecodedOpCode::Halt,
            OpCode::Out { c } => {
                let c = registers.read(c) as u8 as char;
                DecodedOpCode::Out { c: c }
            }
            OpCode::Noop => DecodedOpCode::Noop,
            OpCode::Jmp { addr } => {
                let addr = registers.read(addr).into();
                DecodedOpCode::Jmp { addr: addr }
            }
            OpCode::Jt { cond, addr } => {
                let cond = registers.read(cond);
                let addr = registers.read(addr).into();
                DecodedOpCode::Jt {
                    cond: cond,
                    addr: addr,
                }
            }
            OpCode::Jf { cond, addr } => {
                let cond = registers.read(cond);
                let addr = registers.read(addr).into();
                DecodedOpCode::Jf {
                    cond: cond,
                    addr: addr,
                }
            }
            OpCode::Set { reg, val } => {
                let val = registers.read(val);
                DecodedOpCode::Set {
                    reg: reg,
                    val: val,
                }
            }
            OpCode::Add { reg, val1, val2 } => {
                let val1 = registers.read(val1);
                let val2 = registers.read(val2);
                DecodedOpCode::Add {
                    reg: reg,
                    val1: val1,
                    val2: val2,
                }
            }
            OpCode::Mult { reg, val1, val2 } => {
                let val1 = registers.read(val1);
                let val2 = registers.read(val2);
                DecodedOpCode::Mult {
                    reg: reg,
                    val1: val1,
                    val2: val2,
                }
            }
            OpCode::Mod { reg, val1, val2 } => {
                let val1 = registers.read(val1);
                let val2 = registers.read(val2);
                DecodedOpCode::Mod {
                    reg: reg,
                    val1: val1,
                    val2: val2,
                }
            }
            OpCode::Eq { reg, val1, val2 } => {
                let val1 = registers.read(val1);
                let val2 = registers.read(val2);
                DecodedOpCode::Eq {
                    reg: reg,
                    val1: val1,
                    val2: val2,
                }
            }
            OpCode::Push { val } => {
                let val = registers.read(val);
                DecodedOpCode::Push { val: val }
            }
            OpCode::Pop { reg } => DecodedOpCode::Pop { reg: reg },
            OpCode::Gt { reg, val1, val2 } => {
                let val1 = registers.read(val1);
                let val2 = registers.read(val2);
                DecodedOpCode::Gt {
                    reg: reg,
                    val1: val1,
                    val2: val2,
                }
            }
            OpCode::And { reg, val1, val2 } => {
                let val1 = registers.read(val1);
                let val2 = registers.read(val2);
                DecodedOpCode::And {
                    reg: reg,
                    val1: val1,
                    val2: val2,
                }
            }
            OpCode::Or { reg, val1, val2 } => {
                let val1 = registers.read(val1);
                let val2 = registers.read(val2);
                DecodedOpCode::Or {
                    reg: reg,
                    val1: val1,
                    val2: val2,
                }
            }
            OpCode::Not { reg, val } => {
                let val = registers.read(val);
                DecodedOpCode::Not {
                    reg: reg,
                    val: val,
                }
            }
            OpCode::Call { addr } => {
                let addr = registers.read(addr).into();
                DecodedOpCode::Call { addr: addr }
            }
            OpCode::Rmem { reg, addr } => {
                let addr = registers.read(addr).into();
                DecodedOpCode::Rmem {
                    reg: reg,
                    addr: addr,
                }
            }
            OpCode::Wmem { addr, val } => {
                let addr = registers.read(addr).into();
                let val = registers.read(val);
                DecodedOpCode::Wmem {
                    addr: addr,
                    val: val,
                }
            }
            OpCode::Ret => {
                let addr = ret.map(Addr::from);
                DecodedOpCode::Ret { addr: addr }
            }
            OpCode::In { reg } => DecodedOpCode::In { reg: reg },
        })
    }
}

#[derive(Debug)]
pub enum DecodedOpCode {
    Halt,
    Set { reg: Register, val: u16 },
    Push { val: u16 },
    Pop { reg: Register },
    Eq { reg: Register, val1: u16, val2: u16 },
    Gt { reg: Register, val1: u16, val2: u16 },
    Jmp { addr: Addr },
    Jt { cond: u16, addr: Addr },
    Jf { cond: u16, addr: Addr },
    Add { reg: Register, val1: u16, val2: u16 },
    Mult { reg: Register, val1: u16, val2: u16 },
    Mod { reg: Register, val1: u16, val2: u16 },
    And { reg: Register, val1: u16, val2: u16 },
    Or { reg: Register, val1: u16, val2: u16 },
    Not { reg: Register, val: u16 },
    Rmem { reg: Register, addr: Addr },
    Wmem { addr: Addr, val: u16 },
    Call { addr: Addr },
    Ret { addr: Option<Addr> },
    Out { c: char },
    In { reg: Register },
    Noop,
}

impl fmt::Display for DecodedOpCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        match *self {
            DecodedOpCode::Halt => write!(f, "halt"),
            DecodedOpCode::Set { reg, val } => write!(f, "set {} {}", reg, val),
            DecodedOpCode::Push { val } => write!(f, "push {}", val),
            DecodedOpCode::Pop { reg } => write!(f, "pop {}", reg),
            DecodedOpCode::Eq { reg, val1, val2 } => write!(f, "eq {} {} {}", reg, val1, val2),
            DecodedOpCode::Gt { reg, val1, val2 } => write!(f, "gt {} {} {}", reg, val1, val2),
            DecodedOpCode::Jmp { addr } => write!(f, "jmp {}", addr),
            DecodedOpCode::Jt { cond, addr } => write!(f, "jt {} {}", cond, addr),
            DecodedOpCode::Jf { cond, addr } => write!(f, "jf {} {}", cond, addr),
            DecodedOpCode::Add { reg, val1, val2 } => write!(f, "add {} {} {}", reg, val1, val2),
            DecodedOpCode::Mult { reg, val1, val2 } => write!(f, "mult {} {} {}", reg, val1, val2),
            DecodedOpCode::Mod { reg, val1, val2 } => write!(f, "mod {} {} {}", reg, val1, val2),
            DecodedOpCode::And { reg, val1, val2 } => write!(f, "and {} {} {}", reg, val1, val2),
            DecodedOpCode::Or { reg, val1, val2 } => write!(f, "or {} {} {}", reg, val1, val2),
            DecodedOpCode::Not { reg, val } => write!(f, "not {} {}", reg, val),
            DecodedOpCode::Rmem { reg, addr } => write!(f, "rmem {} {}", reg, addr),
            DecodedOpCode::Wmem { addr, val } => write!(f, "wmem {} {}", addr, val),
            DecodedOpCode::Call { addr } => write!(f, "call {}", addr),
            DecodedOpCode::Ret { addr } if addr.is_some() => write!(f, "ret {}", addr.unwrap()),
            DecodedOpCode::Ret { .. } => write!(f, "ret"),
            DecodedOpCode::Out { c } => write!(f, "out {}", c),
            DecodedOpCode::In { reg } => write!(f, "in {}", reg),
            DecodedOpCode::Noop => write!(f, "noop"),
        }
    }
}

impl OpAccess for DecodedOpCode {
    fn reads(&self, tgt: &Target) -> bool {
        // pretty much all args have already been resolved in DecodeOpCode; the only
        // possible read is from the rmem instruction
        match *self {
            DecodedOpCode::Rmem { ref addr, .. } => tgt == addr,
            _ => false,
        }
    }

    fn writes(&self, tgt: &Target) -> bool {
        match *self {
            DecodedOpCode::Halt => false,
            DecodedOpCode::Set { ref reg, .. } => tgt == reg,
            DecodedOpCode::Push { .. } => false,
            DecodedOpCode::Pop { ref reg } => tgt == reg,
            DecodedOpCode::Eq { ref reg, .. } => tgt == reg,
            DecodedOpCode::Gt { ref reg, .. } => tgt == reg,
            DecodedOpCode::Jmp { .. } => false,
            DecodedOpCode::Jt { .. } => false,
            DecodedOpCode::Jf { .. } => false,
            DecodedOpCode::Add { ref reg, .. } => tgt == reg,
            DecodedOpCode::Mult { ref reg, .. } => tgt == reg,
            DecodedOpCode::Mod { ref reg, .. } => tgt == reg,
            DecodedOpCode::And { ref reg, .. } => tgt == reg,
            DecodedOpCode::Or { ref reg, .. } => tgt == reg,
            DecodedOpCode::Not { ref reg, .. } => tgt == reg,
            DecodedOpCode::Rmem { ref reg, .. } => tgt == reg,
            DecodedOpCode::Wmem { ref addr, .. } => tgt == addr,
            DecodedOpCode::Call { .. } => false,
            DecodedOpCode::Ret { .. } => false,
            DecodedOpCode::Out { .. } => false,
            DecodedOpCode::In { ref reg } => tgt == reg,
            DecodedOpCode::Noop => false,
        }
    }
}
