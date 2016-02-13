extern crate rand;

type Flt = f32;
type Flt2 = (Flt, Flt);

struct RaytraceParams {
    origin: Flt2,
    iters_per_frame: u32,
}

const PARAMS: RaytraceParams = RaytraceParams {
    origin: (10.0, 10.0),
    iters_per_frame: 50,
};

fn next_voxel(pos: Flt2, dir: Flt2) -> Flt2 {
    let tdelta = (1.0 / dir.0, 1.0 / dir.1);
    let tmax = (tdelta.0 * (1.0 - pos.0.fract()),
                tdelta.1 * (1.0 - pos.1.fract()));
    let max = if tmax.0 > 0.0 && tmax.0 < tmax.1 {
        tmax.0
    } else {
        tmax.1
    };
    (pos.0 + dir.0 * max, pos.1 + dir.1 * max)
}

fn validate_bounds(point: (i32, i32), size: (u32, u32)) -> Option<(u32, u32)> {
    if point.0 < 0 || point.0 >= size.0 as i32 || point.1 < 0 || point.1 >= size.1 as i32 {
        None
    } else {
        Some((point.0 as u32, point.1 as u32))
    }
}

fn write_pixel(data: &mut [u8], size: (u32, u32), index: (u32, u32)) {
    let offset = ((index.1 * size.0 + index.0) * 4) as usize;
    data[offset + 0] = data[offset + 0].wrapping_add(1);
    data[offset + 1] = data[offset + 1].wrapping_add(1);
    data[offset + 2] = data[offset + 2].wrapping_add(1);
}

pub fn raytrace(data: &mut [u8], width: u32, height: u32) {
    let mut pos = PARAMS.origin;
    let dir = {
        let theta: Flt = rand::random::<Flt>() * 6.28318530718;
        (theta.cos(), theta.sin())
    };
    loop {
        let floor_pos = (pos.0.floor() as i32, pos.1.floor() as i32);
        if let Some(ipos) = validate_bounds(floor_pos, (width, height)) {
            write_pixel(data, (width, height), ipos);
        } else {
            break;
        }
        pos = next_voxel(pos, dir);
    }
}
