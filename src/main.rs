use std::{thread, time};

use rand::Rng;
use slint::{Image, RenderingState, Rgba8Pixel, SharedPixelBuffer};

slint::include_modules!();

fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;
    // let ui_weak = ui.as_weak();

    let window_clone = ui.clone_strong();

    // ui.window()
    //     .set_rendering_notifier(move |state, _graphics_api| process_rendering(state, &window_clone))
    //     .expect("Couldn't add rendering notifier");

    ui.run()
}

fn process_rendering(state: RenderingState, main_window: &AppWindow) {
    match state {
        RenderingState::AfterRendering => {
            main_window.set_screen_source(render_image(
                main_window.window().size().width,
                main_window.window().size().height,
            ));
        }
        _ => {}
    }
}

fn render_image(window_width: u32, window_height: u32) -> Image {
    let mut pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::new(window_width, window_height);
    let pixels = pixel_buffer.make_mut_slice();

    for i in 0..pixels.len() {
        let r = rand::thread_rng().gen();
        pixels[i] = slint::Rgba8Pixel::new(r, r, r, 255);
    }

    Image::from_rgba8_premultiplied(pixel_buffer)
}
