extern crate sdl2;

use sdl2::event::{Event};
use sdl2::keyboard::{Keycode};
use sdl2::rect::{Rect};
use sdl2::surface::{Surface};
use std::path::{Path};

fn main() {
    // start sdl2

    let ctx = sdl2::init().unwrap();
    let video_ctx = ctx.video().unwrap();
    let mut timer = ctx.timer().unwrap();

    // Create a window

    let mut window = match video_ctx.window("Gaytracer", 640, 480).position_centered()
                                    .opengl().build() {
        Ok(window) => window,
        Err(err)   => panic!("failed to create window: {}", err)
    };

    // Load image
    let img_surface = Surface::load_bmp(&Path::new("circle.bmp")).unwrap();

    // Create a rendering context
    let mut renderer = match window.renderer().build() {
        Ok(renderer) => renderer,
        Err(err) => panic!("failed to create renderer: {}", err)
    };

    // Set the drawing color to a light blue.
    let _ = renderer.set_draw_color(sdl2::pixels::Color::RGB(101, 208, 246));

    // Clear the buffer, using the light blue color set above.
    let _ = renderer.clear();

    // Set the drawing color to a darker blue.
    let _ = renderer.set_draw_color(sdl2::pixels::Color::RGB(0, 153, 204));

    // Create centered Rect, draw the outline of the Rect in our dark blue color.
    let border_rect = Rect::new(320-64, 240-64, 128, 128).unwrap().unwrap();
    let _ = renderer.draw_rect(border_rect);

    // Create a smaller centered Rect, filling it in the same dark blue.
    let inner_rect = Rect::new(320-60, 240-60, 120, 120).unwrap().unwrap();
    let _ = renderer.fill_rect(inner_rect);

    {
        let mut renderer_surface = renderer.surface_mut().unwrap();
        renderer_surface.blit(None, img_surface, None);
    }

    // Swap our buffer for the present buffer, displaying it.
    let _ = renderer.present();

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
