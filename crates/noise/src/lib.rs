//! # aphyn_noise
//!
//! Deterministic, seeded 3D coherent noise sampled on the CPU: [`OpenSimplex2`] (smooth gradient noise) and [`Worley`]
//! (cellular noise). Both are `Copy + Send + Sync` and hold only their seed, so they are cheap to create per call site
//! and safe to share between threads.
//!
//! # How to use
//!
//! Create a noise with a seed and sample it at a point. The same seed and point always give the same value; different
//! seeds give unrelated patterns, so offset the seed to get independent layers.
//!
//! Features are about one unit wide, so divide the input by the desired feature size:
//!
//! ```
//! use aphyn_math::Vec3;
//! use aphyn_noise::{OpenSimplex2, Worley, WorleyMode};
//!
//! let p = Vec3::new(12.0, 3.5, -40.0);
//! let cell_size = 128.0_f32;
//!
//! let simplex = OpenSimplex2::new(7);
//! let s = simplex.sample(p / cell_size);
//! assert!((-1.0..=1.0).contains(&s));
//! assert_eq!(s, OpenSimplex2::new(7).sample(p / cell_size));
//!
//! let worley = Worley::new(7, WorleyMode::EdgeDistance);
//! let w = worley.sample(p / cell_size);
//! assert!((0.0..=1.0).contains(&w));
//! ```
//!
//! #### Output ranges
//!
//! - [`OpenSimplex2::sample`] returns `[-1, 1]` with a mean near 0. It is a port of KdotJPG's OpenSimplex2 (fast
//!   variant, 3D fallback orientation).
//! - [`Worley::sample`] returns `[0, 1]`. It places one jittered feature point per integer cell; [`WorleyMode`] picks
//!   the output:
//!   - `F1` — distance to the nearest feature point;
//!   - `F2MinusF1` — difference between the second-nearest and nearest distances (0 on cell borders);
//!   - `CellValue` — a random constant per cell (flat-shaded cells);
//!   - `EdgeDistance` — exact distance to the nearest cell border, capped at 0.5 and scaled to `[0, 1]` (0 on borders).

mod open_simplex;
mod worley;

pub use open_simplex::*;
pub use worley::*;
