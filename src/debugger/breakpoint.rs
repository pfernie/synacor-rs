use std::{fmt, result};
use std::str::FromStr;

use errors::*;
use memory::{Addr, Target};
use op_code::{DecodedOpCode, OpAccess, OpCode};

#[derive(Debug)]
pub enum Breakpoint {
    At(Addr),
    Read(Target),
    Write(Target),
    Access(Target),
}

impl fmt::Display for Breakpoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        match *self {
            Breakpoint::At(addr) => write!(f, "@ {}", addr),
            Breakpoint::Read(ref t) => write!(f, "read {}", t),
            Breakpoint::Write(ref t) => write!(f, "write {}", t),
            Breakpoint::Access(ref t) => write!(f, "access {}", t),
        }
    }
}

#[derive(Debug)]
pub enum Reason<'bp> {
    Halted,
    Stalled,
    Triggered(&'bp Breakpoint),
}

impl<'bp> fmt::Display for Reason<'bp> {
    fn fmt(&self, f: &mut fmt::Formatter) -> result::Result<(), fmt::Error> {
        match *self {
            Reason::Stalled => write!(f, "machine stalled"),
            Reason::Halted => write!(f, "machine halted"),
            Reason::Triggered(bp) => write!(f, "triggered {}", bp),
        }
    }
}

impl Breakpoint {
    pub fn at(loc: &str) -> Result<Breakpoint> {
        Target::from_str(loc).and_then(|tgt| match tgt {
            Target::Mem(addr) => Ok(Breakpoint::At(addr)),
            _ => bail!("must specify memory address for @ breakpoint"),
        })
    }

    pub fn read(loc: &str) -> Result<Breakpoint> {
        Target::from_str(loc).map(Breakpoint::Read)
    }

    pub fn write(loc: &str) -> Result<Breakpoint> {
        Target::from_str(loc).map(Breakpoint::Write)
    }

    pub fn access(loc: &str) -> Result<Breakpoint> {
        Target::from_str(loc).map(Breakpoint::Access)
    }

    pub fn is_triggered(&self, ip: &Addr, op_code: &OpCode, decoded_op: &DecodedOpCode) -> bool {
        match *self {
            Breakpoint::At(ref addr) => ip == addr,
            Breakpoint::Read(ref t) => op_code.reads(t) || decoded_op.reads(t),
            Breakpoint::Write(ref t) => op_code.writes(t) || decoded_op.writes(t),
            Breakpoint::Access(ref t) => op_code.accesses(t) || decoded_op.accesses(t),
        }
    }
}
