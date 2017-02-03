use memory::Register;
error_chain!{
    foreign_links {
        Io(::std::io::Error);
        ParseInt(::std::num::ParseIntError);
    }

    errors {
        InvalidMemorySize(u: usize) {
            description("Memory image must be 65536 bytes or less and of even size")
            display("Memory image must be 65536 bytes or less and of even size: {} bytes provided", u)
        }
        MachineHalted {
            description("machine halted")
                display("attempted to run halted Machine")
        }
        NonLiteralOpCode(r: Register) {
            description("non-Literal OpCode")
                display("attempted to use register {:?} as OpCode", r)
        }
        EmptyStack {
            description("empty stack")
                display("attempted to pop empty stack")
        }
        InvalidAddr(u: usize) {
            description("invalid Addr")
                display("invalid Addr: 0x{:04x} [0x0..0x7FFF]", u)
        }
        InvalidRegister(u: u16) {
            description("invalid Register")
                display("invalid Register: {}", u)
        }
        InvalidValue(u: u16) {
            description("invalid Value")
                display("invalid Value: {}", u)
        }
        InvalidOpCode(u: u16) {
            description("invalid OpCode")
                display("invalid OpCode: {}", u)
        }
    }
}
