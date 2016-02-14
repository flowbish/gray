extern crate rand;

use std::ops::{Add, Sub, Mul, Div, Neg};
use std::ops::IndexMut;
use std::cmp::max;

type Flt = f64;

#[derive(Debug, Copy, Clone)]
struct Flt2 {
    x: Flt,
    y: Flt,
}

pub struct RaytraceState<'a> {
    buffer: Vec<(Flt, Flt, Flt)>,
    size: (u32, u32),
    orig_buf: &'a [u8],
    blur_buf: &'a [u8],
    bytes_per_pixel: usize,
    origin: (Flt, Flt),
    iters_per_frame: u32,
}

impl<'a> RaytraceState<'a> {
    pub fn new(size: (u32, u32),
               orig_buf: &'a [u8],
               blur_buf: &'a [u8],
               origin: (f64, f64))
               -> RaytraceState<'a> {
        RaytraceState {
            buffer: vec![(0.0, 0.0, 0.0); (size.0 * size.1) as usize],
            size: size,
            orig_buf: orig_buf,
            blur_buf: blur_buf,
            origin: origin,
            bytes_per_pixel: 4,
            iters_per_frame: 1000,
        }
    }
}

impl Flt2 {
    fn new(x: Flt, y: Flt) -> Flt2 {
        Flt2 { x: x, y: y }
    }

    fn fract(self) -> Flt2 {
        Self::new(self.x.fract(), self.y.fract())
    }

    fn floor(self) -> Flt2 {
        Self::new(self.x.floor(), self.y.floor())
    }

    fn floori(self) -> (i32, i32) {
        let floor = self.floor();
        (floor.x as i32, floor.y as i32)
    }

    fn normalized(self) -> Flt2 {
        self / dot(self, self).sqrt()
    }
}

impl PartialEq for Flt2 {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl Eq for Flt2 {}

impl Add for Flt2 {
    type Output = Flt2;

    fn add(self, other: Flt2) -> Flt2 {
        Flt2::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Flt2 {
    type Output = Flt2;

    fn sub(self, other: Flt2) -> Flt2 {
        Flt2::new(self.x - other.x, self.y - other.y)
    }
}

impl Neg for Flt2 {
    type Output = Flt2;

    fn neg(self) -> Flt2 {
        Flt2::new(-self.x, -self.y)
    }
}

impl Mul<Flt> for Flt2 {
    type Output = Flt2;

    fn mul(self, other: Flt) -> Flt2 {
        Flt2::new(self.x * other, self.y * other)
    }
}

impl Div for Flt2 {
    type Output = Flt2;

    fn div(self, other: Flt2) -> Flt2 {
        Flt2::new(self.x / other.x, self.y / other.y)
    }
}

impl Div<Flt> for Flt2 {
    type Output = Flt2;

    fn div(self, other: Flt) -> Flt2 {
        Flt2::new(self.x / other, self.y / other)
    }
}

#[test]
fn test_flt2_ops() {
    fn vecis(v: Flt2, x: Flt, y: Flt) {
        assert!(v.x == x && v.y == y);
    }
    let v11 = Flt2::new(1.0, 1.0);
    let v15 = Flt2::new(1.0, 5.0);
    let add = v11 + v15;
    vecis(add, 2.0, 6.0);
    let sub = v15 - v11;
    vecis(sub, 0.0, 4.0);
    let mul = v15 * 2.0;
    vecis(mul, 2.0, 10.0);
    let div = v15 / 2.0;
    vecis(div, 0.5, 2.5);
    let div2 = v15 / v11;
    vecis(div2, 1.0, 5.0);
}

/// Progresses on to the next pixel along the specified direction.
fn next_voxel(pos: Flt2, dir: Flt2) -> Flt2 {
    fn max_inf(x: Flt, y: Flt) -> Flt {
        if x.is_finite() && y.is_finite() {
            x.max(y)
        } else {
            (2.0 as Flt).sqrt()
        }
    }
    let pos_frac = pos.fract();
    let t1 = (Flt2::new(1.0, 1.0) - pos_frac) / dir;
    let t2 = -pos_frac / dir;
    let tx = max_inf(t1.x, t2.x);
    let ty = max_inf(t1.y, t2.y);
    let dist = tx.min(ty) + 0.01;
    pos + dir * dist
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

fn dot(left: Flt2, right: Flt2) -> Flt {
    left.x * right.x + left.y * right.y
}

fn buf_to_pix(val: Flt) -> u8 {
    // gamma correction
    let val = val.powf(1.0 / 2.2);
    // clamp
    if val >= 1.0 {
        255
    } else if val < 0.0 {
        0
    } else {
        (val * 255.0) as u8
    }
}

// Calculates new ray direction according to diffuse laws
fn diffuse_dir(incoming: Flt2, normal: Flt2) -> Flt2 {
    let result = Flt2::new(rand::random::<Flt>() * 2.0 - 1.0,
                           rand::random::<Flt>() * 2.0 - 1.0)
                     .normalized();
    if (dot(result, normal) < 0.0) != (dot(normal, incoming) < 0.0) {
        -result
    } else {
        result
    }
}

// Calculates new ray direction according to reflection laws
fn reflect_dir(incoming: Flt2, normal: Flt2) -> Flt2 {
    incoming - normal * (2.0 * dot(incoming, normal))
}

// Calculates new ray direction according to refraction laws
fn refract_dir(mut eta: Flt, incoming: Flt2, mut normal: Flt2) -> Option<Flt2> {
    let mut c1 = -dot(incoming, normal);
    if c1 < 0.0 {
        c1 = -c1;
        normal = -normal;
        eta = 1.0 / eta;
    }
    let cs2 = 1.0 - eta * eta * (1.0 - c1 * c1);
    if cs2 < 0.0 {
        None
    } else {
        let normal_mul = eta * c1 - cs2.sqrt();
        let result = incoming * eta + normal * normal_mul;
        Some(result)
    }
}

#[test]
fn test_refract() {
    assert_eq!(refract_dir(2.0, Flt2::new(1.0, 0.0), Flt2::new(-1.0, 0.0)).unwrap(),
               Flt2::new(1.0, 0.0));
    assert_eq!(refract_dir(2.0, Flt2::new(0.0, 1.0), Flt2::new(0.0, 1.0)).unwrap(),
               Flt2::new(0.0, 1.0));
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
        let weight = multiplier / ((self.iters_per_frame * frame) as Flt);

        // Cast out a bunch of rays!
        for _ in 0..self.iters_per_frame {
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
        let mut pos = Flt2::new(self.origin.0, self.origin.1);
        let mut dir = {
            let theta: Flt = rand::random::<Flt>() * 6.28318530718;
            Flt2::new(theta.cos(), theta.sin())
        };

        // Choose a random color for the current ray
        let hue = rand::random::<Flt>();
        let rgb = hue_to_rgb(hue);

        // Keep track of the number of refractions this ray has had
        let mut num_refracts = 0;
        let mut old_value = -1.0; // sentinel value detected in refract for first iteration
        let mut max_iters = self.max_ray_length();

        // Keep following this ray until we end out of bounds
        while let Some(ipos) = validate_bounds(pos.floori(), self.size) {
            if max_iters == 0 {
                break;
            }
            max_iters -= 1;
            if num_refracts != 0 {
                num_refracts -= 1;
            } else {
                match self.refract(&mut dir, &mut old_value, ipos, hue) {
                    Some(true) => num_refracts = 0,
                    Some(false) => (),
                    None => {
                        self.put_pixel(ipos, (10.0, -10.0, -10.0), 1.0);
                        break;
                    }
                }
            }
            self.put_pixel(ipos, rgb, weight);
            pos = next_voxel(pos, dir);
        }
    }

    /// Bend a beam of light as it crosses the boundary between two materials, the
    /// intensity depending on the wavelength (color) of the ray.
    fn refract(&self, dir: &mut Flt2, old: &mut Flt, coords: (u32, u32), hue: Flt) -> Option<bool> {
        // Implemented based on the following papers
        // http://steve.hollasch.net/cgindex/render/refraction.txt
        // http://graphics.stanford.edu/courses/cs148-10-summer/docs/2006--degreve--reflection_refraction.pdf
        let new = self.orig_value_at(coords);
        if !dir.x.is_finite() {
            panic!("non-finite dir");
        }
        // old >= 0 to not do first iteration
        let result = if *old >= 0.0 && (new > 0.5) != (*old > 0.5) {
            let eta = 1.3 + hue * 0.2;
            if let Some(normal) = self.normal_at(coords) {
                let rand_bounce = rand::random::<Flt>();
                let thresh_diffuse = 0.75;
                *dir = if new < 0.5 && rand_bounce > thresh_diffuse {
                    diffuse_dir(*dir, normal)
                } else {
                    refract_dir(eta, *dir, normal).unwrap_or(reflect_dir(*dir, normal))
                };
                *dir = dir.normalized();
            } else {
                *old = new;
                return None;
            }
            true
        } else {
            false
        };
        *old = new;
        Some(result)
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
    fn normal_at(&self, coords: (u32, u32)) -> Option<Flt2> {
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
        let c01 = self.blur_value_at((left, coords.1));
        let c02 = self.blur_value_at((left, down));
        let c10 = self.blur_value_at((coords.0, up));
        let c12 = self.blur_value_at((coords.0, down));
        let c20 = self.blur_value_at((right, up));
        let c21 = self.blur_value_at((right, coords.1));
        let c22 = self.blur_value_at((right, down));
        // Calculate a (very approximate) gradient using Sobel filter
        let x = (c20 + 2.0 * c21 + c22) - (c00 + 2.0 * c01 + c02);
        let y = (c02 + 2.0 * c12 + c22) - (c00 + 2.0 * c10 + c20);
        if x == 0.0 && y == 0.0 {
            None
        } else {
            Some(Flt2::new(x, y).normalized())
        }
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
