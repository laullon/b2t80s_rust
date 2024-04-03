union MyUnion {
    f1: [u8; 2],
    f2: [u16; 6],
}

use b2t80s_rust::z80;

fn main() {
    let mut u1 = MyUnion { f2: [0; 6] };
    unsafe {
        u1.f2[0] = 0x1234;

        println!("{:#04x}", u1.f1[0]);
        println!("{:#04x}", u1.f1[1]);
        println!("{:#04x}", u1.f2[0]);
        println!("{:?}", u1.f2[1]);
    }
    println!("->{}", z80::hello());
}

#[cfg(test)]
mod tests {
    use crate::MyUnion;

    #[test]
    fn internal() {
        let mut u1 = MyUnion { f2: [0; 6] };
        unsafe {
            u1.f2[0] = 0x1234;
            assert_eq!(u1.f1[0], 0x34);
            assert_eq!(u1.f1[1], 0x12);
            assert_eq!(u1.f2[0], 0x1234);
        }
    }
}
