use std::collections::HashMap;
use std::thread;
trait GetFoo {
    fn get_foo(&mut self, u16, u16, u16) -> u16;
}
impl GetFoo for HashMap<(u16, u16), u16> {
    fn get_foo(&mut self, x: u16, y: u16, r7: u16) -> u16 {
        // unwrap_or_else has worse stack behavior, due to closure?
        let v = self.get(&(x, y)).cloned();
        match v {
            Some(v) => v,
            None => {
                let v = foo(x, y, r7, self);
                self.insert((x, y), v);
                v
            }
        }
    }
}
fn foo(x: u16, y: u16, r7: u16, memo: &mut HashMap<(u16, u16), u16>) -> u16 {
    if x != 0 {
        if y != 0 {
            let y2 = memo.get_foo(x, y - 1, r7);
            memo.get_foo(x - 1, y2, r7)
        } else {
            memo.get_foo(x - 1, r7, r7)
        }
    } else {
        y + 1
    }
}

fn do_foo(r7: u16) {
    let mut memo = HashMap::new();
    print!("{}: ", r7);
    let r = foo(4, 1, r7, &mut memo);
    println!("{} ", r);
    if r == 6 {
        panic!("{}: 6!!!!", r7); // yeah, lazy, just die
    }
}

fn main() {
    // spawn a thread to increase stack size...
    // could also multi-thread this thing, but just run it...
    let t = thread::Builder::new().stack_size(1024 * 1024 * 32).spawn(move || {
        for r7 in 0..u16::max_value() {
            do_foo(r7);
        }
        do_foo(u16::max_value());
        println!("done");
    }).unwrap().join().unwrap();
}
