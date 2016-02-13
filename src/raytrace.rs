extern crate rand;

use std::ops::IndexMut;

type Flt = f64;
type Flt2 = (Flt, Flt);

pub struct RaytraceState<'a> {
    buffer: Vec<(Flt, Flt, Flt)>,
    size: (u32, u32),
    orig_buf: &'a [u8],
    blur_buf: &'a [u8],
}

impl<'a> RaytraceState<'a> {
    pub fn new(size: (u32, u32), orig_buf: &'a [u8], blur_buf: &'a [u8]) -> RaytraceState<'a> {
        RaytraceState {
            buffer: vec![(0.0, 0.0, 0.0); (size.0 * size.1) as usize],
            size: size,
            orig_buf: orig_buf,
            blur_buf: blur_buf,
        }
    }
}

struct RaytraceParams {
    origin: Flt2,
    iters_per_frame: u32,
}

const PARAMS: RaytraceParams = RaytraceParams {
    origin: (100.5, 100.5),
    iters_per_frame: 1000,
};

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

fn normalize(val: Flt2) -> Flt2 {
    let len = (val.0 * val.0 * val.1 * val.1).sqrt();
    (val.0 / len, val.1 / len)
}

fn dot(left: Flt2, right: Flt2) -> Flt {
    left.0 * right.0 + left.1 * right.1
}

fn buf_to_pix(val: Flt) -> u8 {
    if val >= 1.0 {
        255
    } else if val < 0.0 {
        0
    } else {
        (val * 255.0) as u8
    }
}

impl<'a> RaytraceState<'a> {
    pub fn raytrace(&mut self, data: &mut [u8], frame: u32) {
        let fixed_frame = frame * PARAMS.iters_per_frame;
        let weight = PARAMS.iters_per_frame as Flt / fixed_frame as Flt;
        for _ in 0..PARAMS.iters_per_frame {
            self.raytrace_single(weight);
        }
        let mulby = 1.0 - weight;
        self.mul(mulby);
        self.blit(data);
    }

    fn raytrace_single(&mut self, weight: Flt) {
        let mut pos = PARAMS.origin;
        let mut dir = {
            let theta: Flt = rand::random::<Flt>() * 6.28318530718;
            (theta.cos(), theta.sin())
        };
        let mut old_value = 0.0;
        loop {
            let floor_pos = (pos.0.floor() as i32, pos.1.floor() as i32);
            if let Some(ipos) = validate_bounds(floor_pos, self.size) {
                self.refract(&mut dir, &mut old_value, ipos);
                let temp = 1.0;
                self.put_pixel(ipos, (temp, temp, temp), weight);
            } else {
                break;
            }
            pos = next_voxel(pos, dir);
        }
    }

    fn refract(&self, dir: &mut Flt2, old: &mut Flt, coords: (u32, u32)) {
        let new = self.orig_value_at(coords);
        if (new > 0.5) != (*old > 0.5) {
            let index_of_refraction = if new > 0.5 {
                2.0
            } else {
                1.0 / 2.0
            };
            let index_of_refraction2 = index_of_refraction * index_of_refraction;
            let normal = self.normal_at(coords);
            let cos_theta_i = -dot(*dir, normal);
            let sin2_theta_t = index_of_refraction2 *
                               (1.0 - cos_theta_i * cos_theta_i);
            let under_sqrt = 1.0 - sin2_theta_t;
            if under_sqrt < 0.0 {
                *old = new;
                return; // TODO
            }
            let refract_i = (dir.0 * index_of_refraction2, dir.1 * index_of_refraction2);
            let normal_mul = index_of_refraction * cos_theta_i + under_sqrt.sqrt();
            let refract_n = (normal.0 * normal_mul, normal.1 * normal_mul);
            *dir = (refract_i.0 + refract_n.0, refract_i.1 + refract_n.1);
        }
        *old = new;
    }

    fn blur_at(&self, coords: (u32, u32)) -> (u8, u8, u8) {
        let idx = (coords.1 * self.size.0 + coords.0) as usize;
        let red = self.blur_buf[idx * 4 + 0];
        let green = self.blur_buf[idx * 4 + 1];
        let blue = self.blur_buf[idx * 4 + 2];
        return (red, green, blue);
    }

    fn blur_value_at(&self, coords: (u32, u32)) -> Flt {
        let blur = self.blur_at(coords);
        return (blur.0 + blur.1 + blur.2) as Flt / (255.0 * 3.0);
    }

    fn orig_at(&self, coords: (u32, u32)) -> (u8, u8, u8) {
        let idx = (coords.1 * self.size.0 + coords.0) as usize;
        let red = self.orig_buf[idx * 4 + 0];
        let green = self.orig_buf[idx * 4 + 1];
        let blue = self.orig_buf[idx * 4 + 2];
        return (red, green, blue);
    }

    fn orig_value_at(&self, coords: (u32, u32)) -> Flt {
        let orig = self.orig_at(coords);
        return (orig.0 + orig.1 + orig.2) as Flt / (255.0 * 3.0);
    }

    fn normal_at(&self, coords: (u32, u32)) -> Flt2 {
        let c00 = self.blur_value_at((coords.0 - 1, coords.1 - 1));
        let c01 = self.blur_value_at((coords.0 - 1, coords.1 + 1));
        let c10 = self.blur_value_at((coords.0 + 1, coords.1 - 1));
        let c11 = self.blur_value_at((coords.0 + 1, coords.1 + 1));
        let x = (c10 - c00) + (c11 - c10);
        let y = (c01 - c00) + (c11 - c01);
        normalize((x, y))
    }

    fn put_pixel(&mut self, coords: (u32, u32), value: (Flt, Flt, Flt), weight: Flt) {
        let index = coords.1 * self.size.0 + coords.0;
        let arr = self.buffer.index_mut(index as usize);
        arr.0 = arr.0 * (1.0 - weight) + value.0 * weight;
        arr.1 = arr.1 * (1.0 - weight) + value.1 * weight;
        arr.2 = arr.2 * (1.0 - weight) + value.2 * weight;
    }

    fn mul(&mut self, value: Flt) {
        for v in self.buffer.iter_mut() {
            v.0 *= value;
            v.1 *= value;
            v.2 *= value;
        }
    }

    fn blit(&self, data: &mut [u8]) {
        for (i, v) in self.buffer.iter().enumerate() {
            data[i * 4 + 0] = buf_to_pix(v.0);
            data[i * 4 + 1] = buf_to_pix(v.1);
            data[i * 4 + 2] = buf_to_pix(v.2);
        }
    }
}
