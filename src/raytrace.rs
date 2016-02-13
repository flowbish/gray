extern crate rand;

use std::ops::IndexMut;

type Flt = f32;
type Flt2 = (Flt, Flt);

pub struct RaytraceState<'a> {
    buffer: Vec<(Flt, Flt, Flt)>,
    size: (u32, u32),
    origBuf: &'a [u8],
    blurBuf: &'a [u8],
}

impl<'a> RaytraceState<'a> {
    pub fn new(size: (u32, u32), origBuf: &'a [u8], blurBuf: &'a [u8]) -> RaytraceState<'a> {
        RaytraceState {
            buffer: vec![(0.0, 0.0, 0.0); (size.0 * size.1) as usize],
            size: size,
            origBuf: origBuf,
            blurBuf: blurBuf,
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
        let dir = {
            let theta: Flt = rand::random::<Flt>() * 6.28318530718;
            (theta.cos(), theta.sin())
        };
        let mut old_pix = (0, 0);
        loop {
            let floor_pos = (pos.0.floor() as i32, pos.1.floor() as i32);
            if let Some(ipos) = validate_bounds(floor_pos, self.size) {
                old_pix = ipos;
                let temp = 1.0;
                self.put_pixel(ipos, (temp, temp, temp), weight);
            } else {
                break;
            }
            pos = next_voxel(pos, dir);
        }
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
