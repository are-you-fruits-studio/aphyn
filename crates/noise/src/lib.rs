//! # aphyn_noise
//!
//! Deterministic, seeded 2D and 3D coherent noise sampled on the CPU: OpenSimplex2 ([`OpenSimplex2D`],
//! [`OpenSimplex3D`], smooth gradient noise) and Worley (cellular) noise, one type per variant and dimension. Every
//! noise is `Copy + Send + Sync` and holds only its seed, so it is cheap to create per call site and safe to share
//! between threads.
//!
//! # How to use
//!
//! Create a noise with a seed and sample it at a point. The same seed and point always give the same result; different
//! seeds give unrelated patterns, so offset the seed to get independent layers.
//!
//! Features are about one unit wide, so divide the input by the desired feature size:
//!
//! ```
//! use aphyn_math::{Vec2, Vec3};
//! use aphyn_noise::*;
//!
//! let p = Vec3::new(12.0, 3.5, -40.0);
//! let cell_size = 128.0_f32;
//!
//! let s = OpenSimplex3D::new(7).sample(p / cell_size);
//! assert!((-1.0..=1.0).contains(&s));
//! assert_eq!(s, OpenSimplex3D::new(7).sample(p / cell_size));
//!
//! let edge = WorleyEdgeDistance3D::new(7).sample(p / cell_size);
//! assert!((0.0..=0.5).contains(&edge.distance));
//!
//! let cell = WorleyCellValue2D::new(7).sample(Vec2::new(p.x, p.z) / cell_size);
//! assert!((0.0..1.0).contains(&cell.value));
//! ```
//!
//! #### OpenSimplex2
//!
//! [`OpenSimplex2D::sample`] and [`OpenSimplex3D::sample`] return `[-1, 1]` with a mean near 0. They are ports of
//! KdotJPG's OpenSimplex2 (fast variant): 2D `noise2` and 3D `noise3_Fallback`. 3D uses a kernel radius of
//! r² = 0.5 instead of the reference's 0.6, which makes it continuous (the reference jumps slightly where its
//! choice of second-closest lattice point flips); its features are a little smaller than the reference's.
//!
//! #### Worley
//!
//! Worley noise places one jittered feature point per integer cell (square in 2D, cube in 3D). The variants return
//! raw values as structs that carry everything the noise already computed, so callers can reuse it. `V` is
//! [`Vec2<f32>`](aphyn_math::Vec2) or [`Vec3<f32>`](aphyn_math::Vec3), and every `point` is an absolute feature
//! position in noise space.
//!
//! | Noise | `sample` returns |
//! |---|---|
//! | [`WorleyClosestPoint2D`] / [`WorleyClosestPoint3D`] | [`ClosestPoint<V>`]: the nearest feature and its squared distance |
//! | [`WorleyClosest2Points2D`] / [`WorleyClosest2Points3D`] | `[ClosestPoint<V>; 2]`: the two nearest features, nearest first |
//! | [`WorleyClosest2PointsDiff2D`] / [`WorleyClosest2PointsDiff3D`] | [`Closest2PointsDiff<V>`]: both, plus `diff` = second minus first distance (0 on cell borders) |
//! | [`WorleyCellValue2D`] / [`WorleyCellValue3D`] | [`CellValue<V>`]: the nearest feature plus a random `value` in `[0, 1)` per cell (flat-shaded cells) |
//! | [`WorleyEdgeDistance2D`] / [`WorleyEdgeDistance3D`] | [`EdgeDistance<V>`]: exact distance to the nearest cell border, capped at 0.5, plus the features on both sides of it |
//! | [`WorleyEnclosedQuadrangle2D`] | [`EnclosedQuadrangle`]: the quadrangle of 4 neighbouring features that encloses the sample, with per-corner weights |
//!
//! Feature search covers the 3×3 (3×3×3) cells around the sample, the usual Worley approximation.
//!
//! [`EdgeDistance::points`] is `[nearest, neighbour]`, where the border between them is the closest one. When no border
//! is within 0.5, `distance` is 0.5 and `neighbour` is the closest one the pruned search found, not necessarily the
//! true one.
//!
//! [`EnclosedQuadrangle`] corners are ordered x fastest: `[c00, c10, c01, c11]`, the features of a 2×2 block of cells.
//! These quads tile the plane, and each is split into two triangles. `weights` are the barycentric weights of the
//! triangle holding the sample, put on its corners (the fourth weight is 0). They sum to 1 and
//! `Σ weights[i] · points[i]` is the sample, so they interpolate per-corner values:
//!
//! ```
//! use aphyn_math::Vec2;
//! use aphyn_noise::WorleyEnclosedQuadrangle2D;
//!
//! let p = Vec2::new(3.3, -7.8);
//! let quad = WorleyEnclosedQuadrangle2D::new(1).sample(p);
//! let rebuilt = (0..4).fold(Vec2::splat(0.0), |acc, i| acc + quad.points[i] * quad.weights[i]);
//! assert!((rebuilt - p).length_sq() < 1e-8);
//! assert!((quad.weights.iter().sum::<f32>() - 1.0).abs() < 1e-5);
//! ```
//!
//! # How to maintain
//!
//! - **Worley core** (`worley/mod.rs`) is generic over the dimension `N` and works on `[f32; N]` / `[i32; N]`. The
//!   private `Space<N>` trait links `Vec2<f32>`/`Vec3<f32>` to their offset tables; the `worley_noise!` macro generates
//!   the public `…2D`/`…3D` types around one generic sample function.
//! - **Hashing**: `hash(cell, seed, salt)`; salts `0..N` are the jitter axes, salt `N` is the cell value. Changing the
//!   hash or salts changes every Worley pattern. All variants share the same feature points for the same seed.
//! - **Offset tables** are x fastest, so `FIRST_PASS[first_pass_index(o)] == o`. `SECOND_PASS` lists the inner 3^N block
//!   first so the edge bound shrinks early.
//! - **Edge distance** scans the 5^N block around the nearest feature and skips cells that can't beat
//!   `min(best, 0.5)`. The cap is what keeps the pruning effective; lifting it needs a wider or unpruned search.
//!   `edge_distance::tests` compares against the unpruned search.
//! - **Enclosed quadrangle**: a quad with min corner `a` spans x in `[a, a + 2)`, so only the 4 quads with min corner
//!   in `{base − 1, base}²` can hold the sample. Quads are simple but may be non-convex, so each is split along its
//!   interior diagonal (c00–c11 when both triangles keep the counter-clockwise winding, c10–c01 otherwise). A 3D
//!   (hexahedron) version was left out: with full jitter a consistent tetrahedral split can invert.

mod open_simplex;
mod worley;

pub use open_simplex::*;
pub use worley::*;
