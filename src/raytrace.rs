extern crate rand;

use std::ops::IndexMut;
use std::cmp::max;

type Flt = f64;
type Flt2 = (Flt, Flt);

pub struct RaytraceState<'a> {
    buffer: Vec<(Flt, Flt, Flt)>,
    size: (u32, u32),
    orig_buf: &'a [u8],
    blur_buf: &'a [u8],
    bytes_per_pixel: usize,
    origin: Flt2,
}

impl<'a> RaytraceState<'a> {
    pub fn new(size: (u32, u32), orig_buf: &'a [u8], blur_buf: &'a [u8], origin: (f64, f64)) -> RaytraceState<'a> {
        RaytraceState {
            buffer: vec![(0.0, 0.0, 0.0); (size.0 * size.1) as usize],
            size: size,
            orig_buf: orig_buf,
            blur_buf: blur_buf,
            origin: origin,
            bytes_per_pixel: 4,
        }
    }
}

struct RaytraceParams {
    iters_per_frame: u32,
}

const PARAMS: RaytraceParams = RaytraceParams { iters_per_frame: 1000 };

/// Progresses on to the next pixel along the specified direction.
fn next_voxel(pos: Flt2, dir: Flt2) -> Flt2 {
    fn max_inf(x: Flt, y: Flt) -> Flt {
        if x.is_finite() && y.is_finite() {
            x.max(y)
        } else {
            (2.0 as Flt).sqrt()
        }
    }
    let pos_frac = (pos.0.fract(), pos.1.fract());
    let tx1 = (1.0 - pos_frac.0) / dir.0;
    let tx2 = -pos_frac.0 / dir.0;
    let ty1 = (1.0 - pos_frac.1) / dir.1;
    let ty2 = -pos_frac.1 / dir.1;
    let tx = max_inf(tx1, tx2);
    let ty = max_inf(ty1, ty2);
    let dist = tx.min(ty) + 0.01;
    (pos.0 + dir.0 * dist, pos.1 + dir.1 * dist)
}

fn validate_bounds(point: (i32, i32), size: (u32, u32)) -> Option<(u32, u32)> {
    if point.0 < 0 || point.0 >= size.0 as i32 || point.1 < 0 || point.1 >= size.1 as i32 {
        None
    } else {
        Some((point.0 as u32, point.1 as u32))
    }
}

/// Convert from a hue (0-1) to a an RGB triplet
/// Colors are maximum saturation.
fn hue_to_rgb(hue: Flt) -> (Flt, Flt, Flt) {
    let hue3 = hue * 3.0;
    let fract = hue3.fract();
    let color = match hue3 as i32 {
        0 => (fract, 0.0, 1.0 - fract),
        1 => (1.0 - fract, fract, 0.0),
        2 => (0.0, 1.0 - fract, fract),
        _ => panic!("Invalid hue {}", hue),
    };
    (color.0.sqrt(), color.1.sqrt(), color.2.sqrt())
}


fn normalize(val: Flt2) -> Flt2 {
    let len = dot(val, val).sqrt();
    (val.0 / len, val.1 / len)
}

fn dot(left: Flt2, right: Flt2) -> Flt {
    left.0 * right.0 + left.1 * right.1
}

fn buf_to_pix(val: Flt) -> u8 {
    let val = val.powf(1.0 / 2.2);
    if val >= 1.0 {
        255
    } else if val < 0.0 {
        0
    } else {
        (val * 255.0) as u8
    }
}

impl<'a> RaytraceState<'a> {
    /// Raytrace multiple rays originating from the origin
    pub fn raytrace(&mut self, data: &mut [u8], frame: u32) {
        // (x + o * frame) / (1 + frame)
        // x / (1 + frame) + o * frame / (1 + frame);

        // Take into account weighting of previous frames when adding current
        // frame.
        let mulby = (frame - 1) as Flt / (frame as Flt);
        self.mul(mulby);
        let multiplier = 100.0;
        let weight = multiplier / ((PARAMS.iters_per_frame * frame) as Flt);

        // Cast out a bunch of rays!
        for _ in 0..PARAMS.iters_per_frame {
            self.raytrace_single(weight);
        }

        // Write pixel data back into shared buffer.
        self.blit(data);
    }

    /// Maximum length of a ray, as a function of the size of the image. This
    /// keeps a ray from reflecting around inside of an object infinitely.
    fn max_ray_length(&self) -> u32 {
        2 * max(self.size.0, self.size.1)
    }

    /// Trace out a single ray in the environment. The ray is cast, leaving
    /// behind a trail of colored light which is then averaged with the paths
    /// left by other rays.
    fn raytrace_single(&mut self, weight: Flt) {
        // Start at origin with a random starting direction
        let mut pos = self.origin;
        let mut dir = {
            let theta: Flt = rand::random::<Flt>() * 6.28318530718;
            (theta.cos(), theta.sin())
        };

        // Choose a random color for the current ray
        let hue = rand::random::<Flt>();
        let rgb = hue_to_rgb(hue);

        // Keep track of the number of refractions this ray has had
        let mut num_refracts = 0;
        let mut old_value = -1.0;
        let mut max_iters = self.max_ray_length();

        // Keep following this ray until we end out of bounds
        while let Some(ipos) = validate_bounds((pos.0.floor() as i32, pos.1.floor() as i32),
                                               self.size) {
            if max_iters == 0 {
                break;
            }
            max_iters -= 1;
            if num_refracts != 0 {
                num_refracts -= 1;
            } else {
                if self.refract(&mut dir, &mut old_value, ipos, hue) {
                    num_refracts = 3;
                }
            }
            self.put_pixel(ipos, rgb, weight);
            pos = next_voxel(pos, dir);
        }
    }

    /// Bend a beam of light as it crosses the boundary between two materials, the
    /// intensity depending on the wavelength (color) of the ray.
    fn refract(&self, dir: &mut Flt2, old: &mut Flt, coords: (u32, u32), hue: Flt) -> bool {
        // Implemented based on the following papers
        // http://steve.hollasch.net/cgindex/render/refraction.txt
        // http://graphics.stanford.edu/courses/cs148-10-summer/docs/2006--degreve--reflection_refraction.pdf
        let new = self.orig_value_at(coords);
        if !dir.0.is_finite() {
            panic!("non-finite dir");
        }
        // old >= 0 to not do first iteration
        let result = if *old >= 0.0 && (new > 0.5) != (*old > 0.5) {
            let eta = 1.1 + hue * 0.2;
            let normal = self.normal_at(coords);
            if !normal.0.is_finite() {
                *old = new;
                return false;
            }
            let c1 = -dot(*dir, normal);
            let under_sqrt = 1.0 - eta * eta * (1.0 - c1 * c1);
            *dir = if under_sqrt < 0.0 {
                (dir.0 + 2.0 * c1 * normal.0, dir.1 + 2.0 * c1 * normal.1)
            } else {
                let normal_mul = eta * c1 - under_sqrt.sqrt();
                let refract_i = (dir.0 * eta, dir.1 * eta);
                let refract_n = (normal.0 * normal_mul, normal.1 * normal_mul);
                (refract_i.0 + refract_n.0, refract_i.1 + refract_n.1)
            };
            *dir = normalize(*dir);
            true
        } else {
            false
        };
        *old = new;
        result
    }

    fn blur_at(&self, coords: (u32, u32)) -> (u8, u8, u8) {
        let idx = (coords.1 * self.size.0 + coords.0) as usize;
        let red = self.blur_buf[idx * self.bytes_per_pixel + 0];
        let green = self.blur_buf[idx * self.bytes_per_pixel + 1];
        let blue = self.blur_buf[idx * self.bytes_per_pixel + 2];
        return (red, green, blue);
    }

    fn blur_value_at(&self, coords: (u32, u32)) -> Flt {
        let blur = self.blur_at(coords);
        return (blur.0 as Flt + blur.1 as Flt + blur.2 as Flt) / (255.0 * 3.0);
    }

    /// Returns the RGB pixel values of the specified coordinate.
    fn orig_at(&self, coords: (u32, u32)) -> (u8, u8, u8) {
        let idx = (coords.1 * self.size.0 + coords.0) as usize;
        let red = self.orig_buf[idx * self.bytes_per_pixel + 0];
        let green = self.orig_buf[idx * self.bytes_per_pixel + 1];
        let blue = self.orig_buf[idx * self.bytes_per_pixel + 2];
        return (red, green, blue);
    }

    /// Calculate the value of the original image at the specified coordinate.
    /// This uses a standard average of the RGB values.
    fn orig_value_at(&self, coords: (u32, u32)) -> Flt {
        let orig = self.orig_at(coords);
        return (orig.0 as Flt + orig.1 as Flt + orig.2 as Flt) / (255.0 * 3.0);
    }

    /// Calculate the normal at the specified coordinate, using the blur mapped
    /// image.
    fn normal_at(&self, coords: (u32, u32)) -> Flt2 {
        let left = if coords.0 == 0 {
            0
        } else {
            coords.0 - 1
        };
        let right = if coords.0 == self.size.0 - 1 {
            self.size.0 - 1
        } else {
            coords.0 + 1
        };
        let up = if coords.1 == 0 {
            0
        } else {
            coords.1 - 1
        };
        let down = if coords.1 == self.size.1 - 1 {
            self.size.1 - 1
        } else {
            coords.1 + 1
        };
        // Grab a diagonal of values from the current poing
        let c00 = self.blur_value_at((left, up));
        let c01 = self.blur_value_at((left, down));
        let c10 = self.blur_value_at((right, up));
        let c11 = self.blur_value_at((right, down));
        // Calculate a (very approximate) gradient
        let x = (c10 - c00) + (c11 - c01);
        let y = (c01 - c00) + (c11 - c10);
        normalize((x, y))
    }

    /// Push a value to the specified pixel coordinate, taking into account the
    /// weight of the previous values pushed.
    fn put_pixel(&mut self, coords: (u32, u32), value: (Flt, Flt, Flt), weight: Flt) {
        let index = coords.1 * self.size.0 + coords.0;
        let arr = self.buffer.index_mut(index as usize);
        arr.0 = arr.0 + value.0 * weight;
        arr.1 = arr.1 + value.1 * weight;
        arr.2 = arr.2 + value.2 * weight;
    }

    fn mul(&mut self, value: Flt) {
        for v in self.buffer.iter_mut() {
            v.0 *= value;
            v.1 *= value;
            v.2 *= value;
        }
    }

    /// Write pixel data to buffer
    fn blit(&self, data: &mut [u8]) {
        for (i, v) in self.buffer.iter().enumerate() {
            data[i * 4 + 0] = buf_to_pix(v.0);
            data[i * 4 + 1] = buf_to_pix(v.1);
            data[i * 4 + 2] = buf_to_pix(v.2);
            data[i * 4 + 3] = 255;
        }
    }
}
