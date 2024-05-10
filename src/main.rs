use b2t80s_rust::zxspectrum::zx48k::Zx48k;

fn main() -> iced::Result {
    iced::program("ZX Spectrum 48K", Zx48k::update, Zx48k::view)
        .subscription(Zx48k::subscription)
        .run()
}
