use aphyn_math::*;

// port of KdotJPG's OpenSimplex2 (fast variant), 3D `noise3_Fallback`
const PRIME_X: i64 = 0x5205402B9270C86F;
const PRIME_Y: i64 = 0x598CD327003817B5;
const PRIME_Z: i64 = 0x5BCC226E9FA0BACB;
const HASH_MULTIPLIER: i64 = 0x53A3F72DEEC546F5;
const SEED_FLIP_3D: i64 = -0x52D547B2E96ED629;
const FALLBACK_ROTATE_3D: f64 = 2.0 / 3.0;
const NORMALIZER_3D: f64 = 0.07969837668935331;
const RSQUARED_3D: f32 = 0.6;
const GRADIENTS_3D: [[f32; 3]; 48] = build_gradients();

#[derive(Clone, Copy)]
pub struct OpenSimplex2 {
    seed: i64,
}

impl OpenSimplex2 {
    pub fn new(seed: u32) -> Self {
        Self { seed: seed as i64 }
    }

    pub fn sample(&self, p: Vec3<f32>) -> f32 {
        let (x, y, z) = (p.x as f64, p.y as f64, p.z as f64);
        let r = FALLBACK_ROTATE_3D * (x + y + z);
        unrotated_base(self.seed, r - x, r - y, r - z).clamp(-1.0, 1.0)
    }
}

fn unrotated_base(mut seed: i64, xr: f64, yr: f64, zr: f64) -> f32 {
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
            value += (a * a) * (a * a) * grad(seed, xrbp, yrbp, zrbp, xri, yri, zri);
        }

        if ax0 >= ay0 && ax0 >= az0 {
            let b = a + ax0 + ax0;
            if b > 1.0 {
                let b = b - 1.0;
                let xp = xrbp.wrapping_sub((x_sign as i64).wrapping_mul(PRIME_X));
                value += (b * b) * (b * b) * grad(seed, xp, yrbp, zrbp, xri + x_sign as f32, yri, zri);
            }
        } else if ay0 > ax0 && ay0 >= az0 {
            let b = a + ay0 + ay0;
            if b > 1.0 {
                let b = b - 1.0;
                let yp = yrbp.wrapping_sub((y_sign as i64).wrapping_mul(PRIME_Y));
                value += (b * b) * (b * b) * grad(seed, xrbp, yp, zrbp, xri, yri + y_sign as f32, zri);
            }
        } else {
            let b = a + az0 + az0;
            if b > 1.0 {
                let b = b - 1.0;
                let zp = zrbp.wrapping_sub((z_sign as i64).wrapping_mul(PRIME_Z));
                value += (b * b) * (b * b) * grad(seed, xrbp, yrbp, zp, xri, yri, zri + z_sign as f32);
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

fn grad(seed: i64, xrvp: i64, yrvp: i64, zrvp: i64, dx: f32, dy: f32, dz: f32) -> f32 {
    let mut hash = (seed ^ xrvp) ^ (yrvp ^ zrvp);
    hash = hash.wrapping_mul(HASH_MULTIPLIER);
    hash ^= hash >> (64 - 8 + 2);
    // the reference repeats the 48 gradients over 256 slots
    let [gx, gy, gz] = GRADIENTS_3D[((hash >> 2) as usize & 255) % 48];
    gx * dx + gy * dy + gz * dz
}

// all sign/axis permutations of (a, a, 1) and (b, c, 0), the same set as the reference table
const fn build_gradients() -> [[f32; 3]; 48] {
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

    fn points() -> impl Iterator<Item = Vec3<f32>> {
        let mut state = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (state >> 40) as f32 / (1u64 << 24) as f32 * 200.0 - 100.0
        };
        (0..20000).map(move |_| Vec3::new(next(), next(), next()))
    }

    fn unclamped(noise: &OpenSimplex2, p: Vec3<f32>) -> f32 {
        let (x, y, z) = (p.x as f64, p.y as f64, p.z as f64);
        let r = FALLBACK_ROTATE_3D * (x + y + z);
        unrotated_base(noise.seed, r - x, r - y, r - z)
    }

    #[test]
    fn raw_output_is_normalized() {
        let noise = OpenSimplex2::new(3);
        let max = points().map(|p| unclamped(&noise, p).abs()).fold(0.0, f32::max);
        assert!(max > 0.6 && max <= 1.01, "max |value| = {max}");
    }

    #[test]
    fn mean_is_near_zero() {
        let noise = OpenSimplex2::new(3);
        let (sum, count) = points().fold((0.0f64, 0), |(s, c), p| (s + noise.sample(p) as f64, c + 1));
        assert!((sum / count as f64).abs() < 0.02);
    }

    #[test]
    fn deterministic_and_seeded() {
        let a = OpenSimplex2::new(3);
        let b = OpenSimplex2::new(3);
        let c = OpenSimplex2::new(4);
        let mut differs = false;
        for p in points().take(200) {
            assert_eq!(a.sample(p), b.sample(p));
            differs |= a.sample(p) != c.sample(p);
        }
        assert!(differs);
    }

    #[test]
    fn continuous() {
        let noise = OpenSimplex2::new(9);
        for p in points().take(2000) {
            let step = noise.sample(p + Vec3::new(1e-3, -1e-3, 1e-3)) - noise.sample(p);
            assert!(step.abs() < 0.02, "jump {step} at {p:?}");
        }
    }

    #[test]
    fn open_simplex_is_send_and_sync() {
        fn check<T: Send + Sync>() {}
        check::<OpenSimplex2>();
    }
}
