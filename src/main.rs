extern crate sdl2;
extern crate sdl2_image;

mod raytrace;
use std::env;
use sdl2::event::{Event};
use sdl2::keyboard::{Keycode};
use sdl2::surface::{Surface};
use sdl2::rwops::{RWops};
use sdl2::pixels::{PixelFormatEnum};
use sdl2_image::{ImageRWops, INIT_PNG, INIT_JPG};
use std::path::{Path};
use sdl2::{SdlResult};

/// Load in a buffer of pixel data from a png file
fn png_data(path: &str) -> SdlResult<((u32, u32), Vec<u8>)> {
    let image_path = Path::new(path);
    let image_rwops = try!(RWops::from_file(&image_path, "r"));
    let mut image_surface = try!(image_rwops.load_png());
    let height = image_surface.height();
    let width = image_surface.width();
    let image_data = image_surface.without_lock_mut().unwrap();
    Ok(((width, height), image_data.to_vec()))
}

fn print_usage(program_name: &str) {
    println!("Usage:\n\t{} input blur", program_name);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 || args.len() > 3 {
        print_usage(&args[0]);
        return;
    }

    // Paths to load the edge and blurred data from
    let image_edge_path = &args[1];
    let image_blur_path = &args[2];

    // Load in image files
    let ((width, height), edge_data) = match png_data(image_edge_path) {
        Ok(vals) => vals,
        Err(_) => { println!("Failed to load image '{}'.", image_edge_path); return; }
    };
    let ((width_, height_), blur_data) = match png_data(image_blur_path) {
        Ok(vals) => vals,
        Err(_) => { println!("Failed to load image '{}'.", image_blur_path); return; }
    };

    // Assert images are same dimensions
    if width != width_ || height != height_ {
        println!("Images must be the same dimensions.");
        return;
    }

    // Start SDL2
    let ctx = sdl2::init().unwrap();
    let video_ctx = ctx.video().unwrap();
    let _image_context = sdl2_image::init(INIT_PNG | INIT_JPG).unwrap();

    // Create a window
    let window = match video_ctx.window("Gaytracer", width, height).position_centered()
                                    .opengl().build() {
        Ok(window) => window,
        Err(err)   => panic!("Failed to create window: {}", err)
    };

    // Create a rendering context
    let mut renderer = match window.renderer().build() {
        Ok(renderer) => renderer,
        Err(err) => panic!("Failed to create renderer: {}", err)
    };

    // Create surface to be drawn on by the raytracer
    let mut my_surface = Surface::new(width, height, PixelFormatEnum::ARGB8888).unwrap();

    // Create a raytracing state and run it a couple times
    let mut state = raytrace::RaytraceState::new((width, height), &edge_data[..], &blur_data[..]);

    let mut events = ctx.event_pump().unwrap();

    // Loop variables, maximum number of raytracing iterations
    let max_iter = 50;
    let mut iter = 1;

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

        if iter < max_iter {
            // update buffer
            my_surface.with_lock_mut(|data: &mut [u8]| {
                state.raytrace(data, iter);
            });
            iter += 1;
        }

        // Copy texture onto renderer buffer
        let my_texture = renderer.create_texture_from_surface(&my_surface).unwrap();
        renderer.copy(&my_texture, None, None);

        // Swap our buffer for the present buffer, displaying it.
        renderer.present();
    }
}
