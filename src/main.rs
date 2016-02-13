extern crate sdl2;
extern crate sdl2_image;

mod raytrace;
use sdl2::event::{Event};
use sdl2::keyboard::{Keycode};
use sdl2::rect::{Rect};
use sdl2::surface::{Surface};
use sdl2::rwops::{RWops};
use sdl2::pixels::{PixelFormatEnum};
use sdl2_image::{ImageRWops, INIT_PNG, INIT_JPG};
use std::path::{Path};
use sdl2::{SdlResult};

fn png_data(path: &str) -> SdlResult<Vec<u8>> {
    let image_path = Path::new(path);
    let image_rwops = try!(RWops::from_file(&image_path, "r"));
    let mut image_surface = try!(image_rwops.load_png());
    let image_data = image_surface.without_lock_mut().unwrap();
    Ok(image_data.to_vec())
}

fn main() {

    // start sdl2
    let ctx = sdl2::init().unwrap();
    let video_ctx = ctx.video().unwrap();
    let _image_context = sdl2_image::init(INIT_PNG | INIT_JPG).unwrap();
    let mut timer = ctx.timer().unwrap();

    // Create a window
    let mut window = match video_ctx.window("Gaytracer", 400, 400).position_centered()
                                    .opengl().build() {
        Ok(window) => window,
        Err(err)   => panic!("failed to create window: {}", err)
    };

    // Create a rendering context
    let mut renderer = match window.renderer().build() {
        Ok(renderer) => renderer,
        Err(err) => panic!("failed to create renderer: {}", err)
    };

    let image_edge_path = "data/circle.png";
    let image_blur_path = "data/circle_blur.png";

    {
        let my_rwops = RWops::from_file(&Path::new("data/circle.png"), "r").unwrap();
        let mut my_surface = my_rwops.load_png().unwrap();
        let width = my_surface.width();
        let height = my_surface.height();
        let circle = png_data(image_edge_path).unwrap();
        let circle_blur = png_data(image_blur_path).unwrap();
        let mut state = raytrace::RaytraceState::new((width, height), &circle[..], &circle_blur[..]);
        my_surface.with_lock_mut(|data: &mut [u8]| {
            for i in 1..50 {
                state.raytrace(data, i);
            }
        });
        let my_texture = renderer.create_texture_from_surface(my_surface).unwrap();

        // Copy texture onto renderer buffer
        let _ = renderer.copy(&my_texture, None, None);

        // Swap our buffer for the present buffer, displaying it.
        let _ = renderer.present();
    }

    let mut events = ctx.event_pump().unwrap();

    // loop until we receive a QuitEvent or escape key pressed
    'event : loop {
        // poll_event returns the most recent event or NoEvent if nothing has happened
        for event in events.poll_iter() {
            match event {
                Event::Quit{..} => break 'event,
                Event::KeyDown{keycode: Option::Some(Keycode::Escape), ..} =>
                    break 'event,
                _ => continue
            }
        }
    }
}
