#[derive(Default, Debug)]
pub enum SignalReq {
    Read,
    Write,
    #[default]
    None,
}

#[derive(Default, Debug)]
pub struct Signals {
    pub addr: u16,
    pub data: u8,
    pub mem: SignalReq,
    pub port: SignalReq,
    pub interrupt: bool,
}
