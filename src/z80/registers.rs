macro_rules! make_reg_functions {
    ($name:ident, $name2:ident, $l:ident, $h:ident) => {
        pub fn $name(self: &Self) -> u16 {
            ((self.$l as u16) << 8) | (self.$h as u16)
        }

        pub fn $name2(self: &mut Self, v: u16) {
            self.$l = ((v >> 8) as u8);
            self.$h = (v as u8);
        }
    };
}

#[derive(Copy, Clone)]
pub struct Registers {
    pub a: u8,
    pub f: Flags,

    pub b: u8,
    pub c: u8,

    pub d: u8,
    pub e: u8,

    pub h: u8,
    pub l: u8,

    pub a_: u8,
    pub f_: Flags,

    pub b_: u8,
    pub c_: u8,

    pub d_: u8,
    pub e_: u8,

    pub h_: u8,
    pub l_: u8,

    pub ix: u16,
    pub iy: u16,

    pub sp: u16,
    pub pc: u16,

    pub m1: bool,
    pub r: u8,
}

impl Registers {
    pub fn new() -> Self {
        Self {
            a: 0,
            f: Flags::default(),
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            a_: 0,
            f_: Flags::default(),
            b_: 0,
            c_: 0,
            d_: 0,
            e_: 0,
            h_: 0,
            l_: 0,
            ix: 0,
            iy: 0,
            sp: 0,
            pc: 0,
            m1: false,
            r: 0,
        }
    }

    make_reg_functions!(bc, set_bc, b, c);
    make_reg_functions!(de, set_de, d, e);
    make_reg_functions!(hl, set_hl, h, l);

    make_reg_functions!(bc_aux, set_bc_aux, b_, c_);
    make_reg_functions!(de_aux, set_de_aux, d_, e_);
    make_reg_functions!(hl_aux, set_hl_aux, h_, l_);

    pub fn af(self: Self) -> u16 {
        ((self.a as u16) << 8) | (self.f.get() as u16)
    }

    pub fn set_af(self: &mut Self, v: u16) {
        self.a = (v >> 8) as u8;
        self.f.set(v as u8);
    }

    pub fn af_aux(self: &Self) -> u16 {
        ((self.a_ as u16) << 8) | (self.f_.get() as u16)
    }

    pub fn set_af_aux(self: &mut Self, v: u16) {
        self.a_ = (v >> 8) as u8;
        self.f_.set(v as u8);
    }

    pub fn get_r(&mut self, r: u8) -> u8 {
        match r {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => self.h,
            5 => self.l,
            7 => self.a,
            _ => panic!("get_r r:{}", r),
        }
    }

    pub fn set_r(&mut self, r: u8, v: u8) {
        match r {
            0 => self.b = v,
            1 => self.c = v,
            2 => self.d = v,
            3 => self.e = v,
            4 => self.h = v,
            5 => self.l = v,
            7 => self.a = v,
            _ => panic!("get_r r:{}", r),
        }
    }

    pub fn get_rp(&mut self, r: u8, alt: bool) -> u16 {
        match r {
            0 => self.bc(),
            1 => self.de(),
            2 => self.hl(),
            3 => {
                if alt {
                    self.af()
                } else {
                    self.sp
                }
            }
            _ => panic!("set_rp r:{}", r),
        }
    }

    pub fn set_rr(&mut self, r: u8, v: u16, alt: bool) {
        match r {
            0 => self.set_bc(v),
            1 => self.set_de(v),
            2 => self.set_hl(v),
            3 => {
                if alt {
                    self.set_af(v)
                } else {
                    self.sp = v
                }
            }
            _ => panic!("set_rp r:{}", r),
        }
    }
}

#[derive(Copy, Clone)]
pub struct Flags {
    pub C: bool,
    pub N: bool,
    pub P: bool,
    pub F3: bool,
    pub H: bool,
    pub F5: bool,
    pub Z: bool,
    pub S: bool,
}

impl Flags {
    pub fn new() -> Self {
        Self {
            C: false,
            N: false,
            P: false,
            F3: false,
            H: false,
            F5: false,
            Z: false,
            S: false,
        }
    }

    pub fn get(self) -> u8 {
        let mut res = 0u8;
        if self.C {
            res |= 0b00000001;
        }
        if self.N {
            res |= 0b00000010;
        }
        if self.P {
            res |= 0b00000100;
        }
        if self.F3 {
            res |= 0b00001000;
        }
        if self.H {
            res |= 0b00010000;
        }
        if self.F5 {
            res |= 0b00100000;
        }
        if self.Z {
            res |= 0b01000000;
        }
        if self.S {
            res |= 0b10000000;
        }
        res
    }

    pub fn set(&mut self, b: u8) {
        self.C = b & 0b00000001 != 0;
        self.N = b & 0b00000010 != 0;
        self.P = b & 0b00000100 != 0;
        self.F3 = b & 0b00001000 != 0;
        self.H = b & 0b00010000 != 0;
        self.F5 = b & 0b00100000 != 0;
        self.Z = b & 0b01000000 != 0;
        self.S = b & 0b10000000 != 0;
    }
}

impl Default for Flags {
    fn default() -> Self {
        Self::new()
    }
}
