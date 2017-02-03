#![recursion_limit = "1024"]

extern crate byteorder;
extern crate env_logger;
#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
extern crate try_from;

mod debugger;
mod errors;
mod machine;
mod memory;
mod op_code;

fn main() {
    env_logger::init().expect("unable to initialize logging");
    let rom_path = std::env::args().nth(1).expect("must specify ROM file");
    if let Err(e) = debugger::debug(rom_path) {
        println!("error: {}", e);

        for e in e.iter().skip(1) {
            println!("caused by: {}", e);
        }

        if let Some(bt) = e.backtrace() {
            println!("backtrace: {:?}", bt);
        }

        ::std::process::exit(1);
    }
}
