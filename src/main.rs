extern crate sdl2;
extern crate sdl2_image;
extern crate image;

mod raytrace;
use std::env;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::surface::Surface;
use sdl2::rwops::RWops;
use sdl2::pixels::PixelFormatEnum;
use sdl2::SdlResult;
use sdl2_image::{ImageRWops, INIT_PNG, INIT_JPG};
use image::{Pixel, ImageBuffer, Rgba, RgbaImage, RgbImage, imageops};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::sync::mpsc;

/// Load in a buffer of pixel data from a png file and run a Gaussian filter
fn blur_data(image: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Vec<u8> {
    let image_copy = image.clone();
    imageops::blur(&image_copy, 4.0);
    image_copy.into_vec()
}

/// Load in a buffer of pixel data from a png file
fn png_data(image: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> Vec<u8> {
    image.clone().into_vec()
}

fn print_usage(program_name: &str) {
    println!("Usage:\n\t{} image", program_name);
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 || args.len() > 2 {
        print_usage(&args[0]);
        return;
    }

    // Paths to load the edge and blurred data from
    let image_path = Path::new(&args[1]);
    let img = match image::open(image_path) {
        Ok(image) => image,
        Err(_) => {
            println!("Failed to load image '{}'", image_path.to_str().unwrap());
            return;
        }
    };

    // Load image data and apply blur filter
    let imgrgba8 = img.to_rgba();
    let (width, height) = imgrgba8.dimensions();
    let image_data = png_data(&imgrgba8);
    let blur_data = blur_data(&imgrgba8);

    // Start SDL2
    let ctx = sdl2::init().unwrap();
    let video_ctx = ctx.video().unwrap();
    let _image_context = sdl2_image::init(INIT_PNG | INIT_JPG).unwrap();

    // Create a window
    let window = match video_ctx.window("Graytracer", width, height)
                                .position_centered()
                                .opengl()
                                .build() {
        Ok(window) => window,
        Err(err) => panic!("Failed to create window: {}", err),
    };

    // Create a rendering context
    let mut renderer = match window.renderer().build() {
        Ok(renderer) => renderer,
        Err(err) => panic!("Failed to create renderer: {}", err),
    };

    // Create surface to be drawn on by the raytracer
    let mut my_surface = Surface::new(width, height, PixelFormatEnum::ARGB8888).unwrap();

    // Create a raytracing state and run it a couple times
    let mut state = raytrace::RaytraceState::new((width, height), &image_data[..], &blur_data[..], (350.0, 350.0));

    let mut events = ctx.event_pump().unwrap();

    // Current frame rendered
    let mut iter = 1;

    // loop until we receive a QuitEvent or escape key pressed
    'event: loop {
        // poll_event returns the most recent event or NoEvent if nothing has happened
        for event in events.poll_iter() {
            match event {
                Event::Quit{..} |
                Event::KeyDown{keycode: Option::Some(Keycode::Escape), ..} =>
                    break 'event,
                _ => continue
            }
        }

        // update buffer
        my_surface.with_lock_mut(|data: &mut [u8]| {
            state.raytrace(data, iter);
        });
        iter += 1;

        // Copy texture onto renderer buffer
        let my_texture = renderer.create_texture_from_surface(&my_surface).unwrap();
        renderer.copy(&my_texture, None, None);

        // Swap our buffer for the present buffer, displaying it.
        renderer.present();
    }
}
