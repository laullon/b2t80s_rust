use b2t80s_rust::zxspectrum::zx48k::Zx48k;
use std::panic;
use std::process;

fn main() -> iced::Result {
    // take_hook() returns the default hook in case when a custom one is not set
    let orig_hook = panic::take_hook();
    panic::set_hook(Box::new(move |panic_info| {
        // invoke the default handler and exit the process
        orig_hook(panic_info);
        process::exit(1);
    }));

    iced::program("ZX Spectrum 48K", Zx48k::update, Zx48k::view)
        .subscription(Zx48k::subscription)
        .run()
}
