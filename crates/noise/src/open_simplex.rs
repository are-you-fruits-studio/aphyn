use aphyn_math::*;

// ports of KdotJPG's OpenSimplex2 (fast variant): 2D `noise2` and 3D `noise3_Fallback`
const PRIME_X: i64 = 0x5205402B9270C86F;
const PRIME_Y: i64 = 0x598CD327003817B5;
const PRIME_Z: i64 = 0x5BCC226E9FA0BACB;
const HASH_MULTIPLIER: i64 = 0x53A3F72DEEC546F5;
const SEED_FLIP_3D: i64 = -0x52D547B2E96ED629;
const FALLBACK_ROTATE_3D: f64 = 2.0 / 3.0;
const NORMALIZER_3D: f64 = 0.07969837668935331;
const RSQUARED_3D: f32 = 0.6;
const GRADIENTS_3D: [[f32; 3]; 48] = build_gradients_3d();
const SKEW_2D: f64 = 0.366025403784439;
const UNSKEW_2D: f64 = -0.21132486540518713;
const NORMALIZER_2D: f64 = 0.01001634121365712;
const RSQUARED_2D: f32 = 0.5;
// the reference table before normalization
const GRADIENTS_2D_RAW: [[f64; 2]; 24] = [
    [0.38268343236509, 0.923879532511287],
    [0.923879532511287, 0.38268343236509],
    [0.923879532511287, -0.38268343236509],
    [0.38268343236509, -0.923879532511287],
    [-0.38268343236509, -0.923879532511287],
    [-0.923879532511287, -0.38268343236509],
    [-0.923879532511287, 0.38268343236509],
    [-0.38268343236509, 0.923879532511287],
    [0.130526192220052, 0.99144486137381],
    [0.608761429008721, 0.793353340291235],
    [0.793353340291235, 0.608761429008721],
    [0.99144486137381, 0.130526192220051],
    [0.99144486137381, -0.130526192220051],
    [0.793353340291235, -0.60876142900872],
    [0.608761429008721, -0.793353340291235],
    [0.130526192220052, -0.99144486137381],
    [-0.130526192220052, -0.99144486137381],
    [-0.608761429008721, -0.793353340291235],
    [-0.793353340291235, -0.608761429008721],
    [-0.99144486137381, -0.130526192220052],
    [-0.99144486137381, 0.130526192220051],
    [-0.793353340291235, 0.608761429008721],
    [-0.608761429008721, 0.793353340291235],
    [-0.130526192220052, 0.99144486137381],
];
const GRADIENTS_2D: [[f32; 2]; 24] = build_gradients_2d();

#[derive(Clone, Copy)]
pub struct OpenSimplex2D {
    seed: i64,
}

impl OpenSimplex2D {
    pub fn new(seed: u32) -> Self {
        Self { seed: seed as i64 }
    }

    pub fn sample(&self, p: Vec2<f32>) -> f32 {
        let (x, y) = (p.x as f64, p.y as f64);
        let s = SKEW_2D * (x + y);
        unskewed_base_2d(self.seed, x + s, y + s).clamp(-1.0, 1.0)
    }
}

fn unskewed_base_2d(seed: i64, xs: f64, ys: f64) -> f32 {
    let (xsb, ysb) = (xs.floor() as i64, ys.floor() as i64);
    let xi = (xs - xsb as f64) as f32;
    let yi = (ys - ysb as f64) as f32;

    let xsbp = xsb.wrapping_mul(PRIME_X);
    let ysbp = ysb.wrapping_mul(PRIME_Y);

    let t = (xi + yi) * UNSKEW_2D as f32;
    let dx0 = xi + t;
    let dy0 = yi + t;

    let mut value = 0.0;
    let a0 = RSQUARED_2D - dx0 * dx0 - dy0 * dy0;
    if a0 > 0.0 {
        value = (a0 * a0) * (a0 * a0) * grad_2d(seed, xsbp, ysbp, dx0, dy0);
    }

    let a1 = (2.0 * (1.0 + 2.0 * UNSKEW_2D) * (1.0 / UNSKEW_2D + 2.0)) as f32 * t
        + ((-2.0 * (1.0 + 2.0 * UNSKEW_2D) * (1.0 + 2.0 * UNSKEW_2D)) as f32 + a0);
    if a1 > 0.0 {
        let dx1 = dx0 - (1.0 + 2.0 * UNSKEW_2D) as f32;
        let dy1 = dy0 - (1.0 + 2.0 * UNSKEW_2D) as f32;
        value += (a1 * a1) * (a1 * a1) * grad_2d(seed, xsbp.wrapping_add(PRIME_X), ysbp.wrapping_add(PRIME_Y), dx1, dy1);
    }

    let (dx2, dy2, xp, yp) = match dy0 > dx0 {
        true => (
            dx0 - UNSKEW_2D as f32,
            dy0 - (UNSKEW_2D + 1.0) as f32,
            xsbp,
            ysbp.wrapping_add(PRIME_Y),
        ),
        false => (
            dx0 - (UNSKEW_2D + 1.0) as f32,
            dy0 - UNSKEW_2D as f32,
            xsbp.wrapping_add(PRIME_X),
            ysbp,
        ),
    };
    let a2 = RSQUARED_2D - dx2 * dx2 - dy2 * dy2;
    if a2 > 0.0 {
        value += (a2 * a2) * (a2 * a2) * grad_2d(seed, xp, yp, dx2, dy2);
    }

    value
}

fn grad_2d(seed: i64, xsvp: i64, ysvp: i64, dx: f32, dy: f32) -> f32 {
    let mut hash = seed ^ xsvp ^ ysvp;
    hash = hash.wrapping_mul(HASH_MULTIPLIER);
    hash ^= hash >> (64 - 7 + 1);
    // the reference repeats the 24 gradients over 128 slots
    let slot = (hash as usize & 254) >> 1;
    let [gx, gy] = GRADIENTS_2D[slot % 24];
    gx * dx + gy * dy
}

const fn build_gradients_2d() -> [[f32; 2]; 24] {
    let mut out = [[0.0; 2]; 24];
    let mut i = 0;
    while i < 24 {
        out[i] = [
            (GRADIENTS_2D_RAW[i][0] / NORMALIZER_2D) as f32,
            (GRADIENTS_2D_RAW[i][1] / NORMALIZER_2D) as f32,
        ];
        i += 1;
    }
    out
}

#[derive(Clone, Copy)]
pub struct OpenSimplex3D {
    seed: i64,
}

impl OpenSimplex3D {
    pub fn new(seed: u32) -> Self {
        Self { seed: seed as i64 }
    }

    pub fn sample(&self, p: Vec3<f32>) -> f32 {
        let (x, y, z) = (p.x as f64, p.y as f64, p.z as f64);
        let r = FALLBACK_ROTATE_3D * (x + y + z);
        unrotated_base_3d(self.seed, r - x, r - y, r - z).clamp(-1.0, 1.0)
    }
}

fn unrotated_base_3d(mut seed: i64, xr: f64, yr: f64, zr: f64) -> f32 {
    let (xrb, yrb, zrb) = (xr.round() as i64, yr.round() as i64, zr.round() as i64);
    let mut xri = (xr - xrb as f64) as f32;
    let mut yri = (yr - yrb as f64) as f32;
    let mut zri = (zr - zrb as f64) as f32;

    // -1 if positive, 1 if negative
    let mut x_sign = (-1.0 - xri) as i32 | 1;
    let mut y_sign = (-1.0 - yri) as i32 | 1;
    let mut z_sign = (-1.0 - zri) as i32 | 1;

    let mut ax0 = x_sign as f32 * -xri;
    let mut ay0 = y_sign as f32 * -yri;
    let mut az0 = z_sign as f32 * -zri;

    let mut xrbp = xrb.wrapping_mul(PRIME_X);
    let mut yrbp = yrb.wrapping_mul(PRIME_Y);
    let mut zrbp = zrb.wrapping_mul(PRIME_Z);

    let mut value = 0.0;
    let mut a = (RSQUARED_3D - xri * xri) - (yri * yri + zri * zri);

    // one pass per lattice copy
    for lattice in 0..2 {
        if a > 0.0 {
            value += (a * a) * (a * a) * grad_3d(seed, xrbp, yrbp, zrbp, xri, yri, zri);
        }

        if ax0 >= ay0 && ax0 >= az0 {
            let b = a + ax0 + ax0;
            if b > 1.0 {
                let b = b - 1.0;
                let xp = xrbp.wrapping_sub((x_sign as i64).wrapping_mul(PRIME_X));
                value += (b * b) * (b * b) * grad_3d(seed, xp, yrbp, zrbp, xri + x_sign as f32, yri, zri);
            }
        } else if ay0 > ax0 && ay0 >= az0 {
            let b = a + ay0 + ay0;
            if b > 1.0 {
                let b = b - 1.0;
                let yp = yrbp.wrapping_sub((y_sign as i64).wrapping_mul(PRIME_Y));
                value += (b * b) * (b * b) * grad_3d(seed, xrbp, yp, zrbp, xri, yri + y_sign as f32, zri);
            }
        } else {
            let b = a + az0 + az0;
            if b > 1.0 {
                let b = b - 1.0;
                let zp = zrbp.wrapping_sub((z_sign as i64).wrapping_mul(PRIME_Z));
                value += (b * b) * (b * b) * grad_3d(seed, xrbp, yrbp, zp, xri, yri, zri + z_sign as f32);
            }
        }

        if lattice == 1 {
            break;
        }

        ax0 = 0.5 - ax0;
        ay0 = 0.5 - ay0;
        az0 = 0.5 - az0;

        xri = x_sign as f32 * ax0;
        yri = y_sign as f32 * ay0;
        zri = z_sign as f32 * az0;

        a += (0.75 - ax0) - (ay0 + az0);

        xrbp = xrbp.wrapping_add((x_sign >> 1) as i64 & PRIME_X);
        yrbp = yrbp.wrapping_add((y_sign >> 1) as i64 & PRIME_Y);
        zrbp = zrbp.wrapping_add((z_sign >> 1) as i64 & PRIME_Z);

        x_sign = -x_sign;
        y_sign = -y_sign;
        z_sign = -z_sign;

        seed ^= SEED_FLIP_3D;
    }

    value
}

fn grad_3d(seed: i64, xrvp: i64, yrvp: i64, zrvp: i64, dx: f32, dy: f32, dz: f32) -> f32 {
    let mut hash = (seed ^ xrvp) ^ (yrvp ^ zrvp);
    hash = hash.wrapping_mul(HASH_MULTIPLIER);
    hash ^= hash >> (64 - 8 + 2);
    // the reference repeats the 48 gradients over 256 slots
    let [gx, gy, gz] = GRADIENTS_3D[((hash >> 2) as usize & 255) % 48];
    gx * dx + gy * dy + gz * dz
}

// all sign/axis permutations of (a, a, 1) and (b, c, 0), the same set as the reference table
const fn build_gradients_3d() -> [[f32; 3]; 48] {
    const A: f64 = 2.22474487139;
    const B: f64 = 3.0862664687972017;
    const C: f64 = 1.1721513422464978;

    let mut out = [[0.0; 3]; 48];
    let mut n = 0;

    let mut unit_axis = 0;
    while unit_axis < 3 {
        let mut signs = 0;
        while signs < 8 {
            let mut i = 0;
            while i < 3 {
                let magnitude = if i == unit_axis { 1.0 } else { A };
                let sign = if (signs >> i) & 1 == 1 { -1.0 } else { 1.0 };
                out[n][i] = (sign * magnitude / NORMALIZER_3D) as f32;
                i += 1;
            }
            n += 1;
            signs += 1;
        }
        unit_axis += 1;
    }

    let mut b_axis = 0;
    while b_axis < 3 {
        let mut c_axis = 0;
        while c_axis < 3 {
            if c_axis != b_axis {
                let mut signs = 0;
                while signs < 4 {
                    let b_sign = if signs & 1 == 1 { -1.0 } else { 1.0 };
                    let c_sign = if signs & 2 == 2 { -1.0 } else { 1.0 };
                    out[n][b_axis] = (b_sign * B / NORMALIZER_3D) as f32;
                    out[n][c_axis] = (c_sign * C / NORMALIZER_3D) as f32;
                    n += 1;
                    signs += 1;
                }
            }
            c_axis += 1;
        }
        b_axis += 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn points_3d() -> impl Iterator<Item = Vec3<f32>> {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 40) as f32 / (1u64 << 24) as f32 * 200.0 - 100.0
        };
        (0..20000).map(move |_| Vec3::new(next(), next(), next()))
    }

    fn unclamped(noise: &OpenSimplex3D, p: Vec3<f32>) -> f32 {
        let (x, y, z) = (p.x as f64, p.y as f64, p.z as f64);
        let r = FALLBACK_ROTATE_3D * (x + y + z);
        unrotated_base_3d(noise.seed, r - x, r - y, r - z)
    }

    #[test]
    fn raw_output_3d_is_normalized() {
        let noise = OpenSimplex3D::new(3);
        let max = points_3d().map(|p| unclamped(&noise, p).abs()).fold(0.0, f32::max);
        assert!(max > 0.6 && max <= 1.01, "max |value| = {max}");
    }

    #[test]
    fn mean_3d_is_near_zero() {
        let noise = OpenSimplex3D::new(3);
        let (sum, count) = points_3d().fold((0.0f64, 0), |(s, c), p| (s + noise.sample(p) as f64, c + 1));
        assert!((sum / count as f64).abs() < 0.02);
    }

    #[test]
    fn deterministic_and_seeded_3d() {
        let a = OpenSimplex3D::new(3);
        let b = OpenSimplex3D::new(3);
        let c = OpenSimplex3D::new(4);
        let mut differs = false;
        for p in points_3d().take(200) {
            assert_eq!(a.sample(p), b.sample(p));
            differs |= a.sample(p) != c.sample(p);
        }
        assert!(differs);
    }

    #[test]
    fn continuous_3d() {
        let noise = OpenSimplex3D::new(9);
        for p in points_3d().take(2000) {
            let step = noise.sample(p + Vec3::new(1e-3, -1e-3, 1e-3)) - noise.sample(p);
            assert!(step.abs() < 0.02, "jump {step} at {p:?}");
        }
    }

    fn points_2d() -> impl Iterator<Item = Vec2<f32>> {
        let mut coords = points_3d().flat_map(|p| p.into_array());
        (0..20000).map(move |_| Vec2::new(coords.next().unwrap(), coords.next().unwrap()))
    }

    fn unclamped_2d(noise: &OpenSimplex2D, p: Vec2<f32>) -> f32 {
        let (x, y) = (p.x as f64, p.y as f64);
        let s = SKEW_2D * (x + y);
        unskewed_base_2d(noise.seed, x + s, y + s)
    }

    #[test]
    fn raw_output_2d_is_normalized() {
        let noise = OpenSimplex2D::new(3);
        let max = points_2d().map(|p| unclamped_2d(&noise, p).abs()).fold(0.0, f32::max);
        assert!(max > 0.6 && max <= 1.01, "max |value| = {max}");
    }

    #[test]
    fn mean_2d_is_near_zero() {
        let noise = OpenSimplex2D::new(3);
        let (sum, count) = points_2d().fold((0.0f64, 0), |(s, c), p| (s + noise.sample(p) as f64, c + 1));
        assert!((sum / count as f64).abs() < 0.02);
    }

    #[test]
    fn deterministic_and_seeded_2d() {
        let a = OpenSimplex2D::new(3);
        let b = OpenSimplex2D::new(3);
        let c = OpenSimplex2D::new(4);
        let mut differs = false;
        for p in points_2d().take(200) {
            assert_eq!(a.sample(p), b.sample(p));
            differs |= a.sample(p) != c.sample(p);
        }
        assert!(differs);
    }

    #[test]
    fn continuous_2d() {
        let noise = OpenSimplex2D::new(9);
        for p in points_2d().take(2000) {
            let step = noise.sample(p + Vec2::new(1e-3, -1e-3)) - noise.sample(p);
            assert!(step.abs() < 0.02, "jump {step} at {p:?}");
        }
    }

    #[test]
    fn is_send_and_sync() {
        fn check<T: Send + Sync>() {}
        check::<OpenSimplex2D>();
        check::<OpenSimplex3D>();
    }
}
