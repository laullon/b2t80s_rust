macro_rules! make_reg_functions {
    ($name:ident, $name2:ident, $l:ident, $h:ident) => {
        fn $name(self: &Self) -> u16 {
            ((self.$l as u16) << 8) | (self.$h as u16)
        }

        fn $name2(self: &mut Self, v: u16) {
            self.$l = ((v >> 8) as u8);
            self.$h = (v as u8);
        }
    };
}

#[derive(Debug, Copy, Clone)]
pub enum IndexMode {
    Hl,
    Ix,
    Iy,
}

#[derive(Copy, Clone)]
pub struct Registers {
    pub a: u8,
    pub f: Flags,

    pub b: u8,
    pub c: u8,

    d: u8,
    e: u8,

    h: u8,
    l: u8,

    a_: u8,
    f_: Flags,

    b_: u8,
    c_: u8,

    d_: u8,
    e_: u8,

    h_: u8,
    l_: u8,

    ixl: u8,
    ixh: u8,

    iyl: u8,
    iyh: u8,

    pub sp: u16,
    pub pc: u16,

    pub m1: bool,
    pub r: u8,
    pub i: u8,
    pub index_mode: IndexMode,

    pub iff1: bool,
    pub iff2: bool,

    pub im: u8,
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
            ixh: 0,
            ixl: 0,
            iyh: 0,
            iyl: 0,
            sp: 0,
            pc: 0,
            m1: false,
            r: 0,
            i: 0,
            index_mode: IndexMode::Hl,
            iff1: false,
            iff2: false,
            im: 0,
        }
    }

    make_reg_functions!(bc, set_bc, b, c);
    make_reg_functions!(de, set_de, d, e);
    make_reg_functions!(hl, set_hl, h, l);
    make_reg_functions!(ix, set_ix, ixh, ixl);
    make_reg_functions!(iy, set_iy, iyh, iyl);

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

    pub fn get_r(&self, r: u8) -> u8 {
        match r {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => match self.index_mode {
                IndexMode::Hl => self.h,
                IndexMode::Ix => self.ixh,
                IndexMode::Iy => self.iyh,
            },
            5 => match self.index_mode {
                IndexMode::Hl => self.l,
                IndexMode::Ix => self.ixl,
                IndexMode::Iy => self.iyl,
            },
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
            4 => match self.index_mode {
                IndexMode::Hl => self.h = v,
                IndexMode::Ix => self.ixh = v,
                IndexMode::Iy => self.iyh = v,
            },
            5 => match self.index_mode {
                IndexMode::Hl => self.l = v,
                IndexMode::Ix => self.ixl = v,
                IndexMode::Iy => self.iyl = v,
            },
            7 => self.a = v,
            _ => panic!("get_r r:{}", r),
        }
    }

    pub fn get_rr(&self, r: u8) -> u16 {
        match r {
            0 => self.bc(),
            1 => self.de(),
            2 => match self.index_mode {
                IndexMode::Hl => self.hl(),
                IndexMode::Ix => self.ix(),
                IndexMode::Iy => self.iy(),
            },
            3 => self.sp,
            _ => panic!("get_rr r:{}", r),
        }
    }

    pub fn get_rr2(&self, r: u8) -> u16 {
        match r {
            3 => self.af(),
            _ => self.get_rr(r),
        }
    }

    pub fn get_idx(&self, d: u8) -> u16 {
        let mut v;
        match self.index_mode {
            IndexMode::Ix => v = self.ix(),
            IndexMode::Iy => v = self.iy(),
            _ => panic!("get_idx"),
        }
        v = (v as i16).wrapping_add((d as i8) as i16) as u16;
        v
    }

    pub fn set_rr(&mut self, r: u8, v: u16) {
        match r {
            0 => self.set_bc(v),
            1 => self.set_de(v),
            2 => match self.index_mode {
                IndexMode::Hl => self.set_hl(v),
                IndexMode::Ix => self.set_ix(v),
                IndexMode::Iy => self.set_iy(v),
            },
            3 => self.sp = v,
            _ => panic!("set_rp r:{}", r),
        }
    }

    pub fn set_rr2(&mut self, r: u8, v: u16) {
        match r {
            3 => self.set_af(v),
            _ => self.set_rr(r, v),
        }
    }

    pub fn exafaf(&mut self) {
        (self.a, self.a_) = (self.a_, self.a);
        (self.f, self.f_) = (self.f_, self.f);
    }

    pub fn exx(&mut self) {
        (self.b, self.b_) = (self.b_, self.b);
        (self.c, self.c_) = (self.c_, self.c);
        (self.d, self.d_) = (self.d_, self.d);
        (self.e, self.e_) = (self.e_, self.e);
        (self.h, self.h_) = (self.h_, self.h);
        (self.l, self.l_) = (self.l_, self.l);
    }

    pub fn set_all_regs(&mut self, registers: [u16; 12]) {
        self.set_af(registers[0]);
        self.set_bc(registers[1]);
        self.set_de(registers[2]);
        self.set_hl(registers[3]);
        self.set_af_aux(registers[4]);
        self.set_bc_aux(registers[5]);
        self.set_de_aux(registers[6]);
        self.set_hl_aux(registers[7]);
        self.set_ix(registers[8]);
        self.set_iy(registers[9]);
        self.sp = registers[10];
        self.pc = registers[11];
    }

    pub fn dump_registers(&self) -> String {
        format!(
            "{:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x} {:04x}",
            self.af(),
            self.bc(),
            self.de(),
            self.hl(),
            self.af_aux(),
            self.bc_aux(),
            self.de_aux(),
            self.hl_aux(),
            self.ix(),
            self.iy(),
            self.sp,
            self.pc,
        )
    }
}

#[derive(Copy, Clone)]
pub struct Flags {
    pub c: bool,
    pub n: bool,
    pub p: bool,
    pub f3: bool,
    pub h: bool,
    pub f5: bool,
    pub z: bool,
    pub s: bool,
}

impl Flags {
    pub fn new() -> Self {
        Self {
            c: false,
            n: false,
            p: false,
            f3: false,
            h: false,
            f5: false,
            z: false,
            s: false,
        }
    }

    pub fn get(self) -> u8 {
        let mut res = 0u8;
        if self.c {
            res |= 0b00000001;
        }
        if self.n {
            res |= 0b00000010;
        }
        if self.p {
            res |= 0b00000100;
        }
        if self.f3 {
            res |= 0b00001000;
        }
        if self.h {
            res |= 0b00010000;
        }
        if self.f5 {
            res |= 0b00100000;
        }
        if self.z {
            res |= 0b01000000;
        }
        if self.s {
            res |= 0b10000000;
        }
        res
    }

    pub fn set(&mut self, b: u8) {
        self.c = b & 0b00000001 != 0;
        self.n = b & 0b00000010 != 0;
        self.p = b & 0b00000100 != 0;
        self.f3 = b & 0b00001000 != 0;
        self.h = b & 0b00010000 != 0;
        self.f5 = b & 0b00100000 != 0;
        self.z = b & 0b01000000 != 0;
        self.s = b & 0b10000000 != 0;
    }
}

impl Default for Flags {
    fn default() -> Self {
        Self::new()
    }
}
