use aphyn_math::{Quat, Vec3};

use crate::{CollisionAabb, CollisionBox, CollisionLine, CollisionShape, CollisionSphere, LineBoundType};

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct ContactPoint {
    pub position: Vec3<f32>,
    pub depth: f32,
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Debug, Default)]
pub struct ContactManifold {
    pub normal: Vec3<f32>,
    points: [ContactPoint; ContactManifold::MAX_POINTS],
    count: u8,
}

impl ContactManifold {
    pub const MAX_POINTS: usize = 4;

    pub fn new(normal: Vec3<f32>, points: &[ContactPoint]) -> Self {
        let mut manifold = Self {
            normal,
            points: Default::default(),
            count: 0,
        };
        for &p in reduce_points(points).iter().take(Self::MAX_POINTS) {
            manifold.points[manifold.count as usize] = p;
            manifold.count += 1;
        }
        manifold
    }

    pub fn points(&self) -> &[ContactPoint] {
        &self.points[..self.count as usize]
    }

    pub fn max_depth(&self) -> f32 {
        self.points().iter().fold(0.0, |d, p| d.max(p.depth))
    }

    pub fn flipped(mut self) -> Self {
        self.normal = -self.normal;
        self
    }
}

pub fn collide(lhs: CollisionShape, rhs: CollisionShape) -> Option<ContactManifold> {
    use CollisionShape as S;

    let manifold = match (normalize_shape(lhs), normalize_shape(rhs)) {
        (S::Sphere(a), S::Sphere(b)) => collide_sp_sp(a, b),
        (S::Sphere(a), b) => collide_sp_shape(a, b),
        (a, S::Sphere(b)) => collide_sp_shape(b, a).map(ContactManifold::flipped),
        (a, b) => collide_hulls(Hull::from_shape(a)?, Hull::from_shape(b)?),
    }?;

    (manifold.count > 0).then_some(manifold)
}

fn normalize_shape(shape: CollisionShape) -> CollisionShape {
    match shape {
        CollisionShape::Aabb(aabb) => CollisionShape::Box(CollisionBox {
            center: aabb.center,
            extents: aabb.extents,
            rotation: Quat::IDENTITY,
        }),
        shape => shape,
    }
}

// sphere

fn collide_sp_sp(a: CollisionSphere, b: CollisionSphere) -> Option<ContactManifold> {
    let d = b.center - a.center;
    let dist = len(d);
    let depth = a.radius + b.radius - dist;
    if depth < 0.0 {
        return None;
    }

    let normal = if dist > EPS { d / dist } else { Vec3::new(0.0, 1.0, 0.0) };
    let position = a.center + normal * (a.radius - depth * 0.5);

    Some(ContactManifold::new(normal, &[ContactPoint { position, depth }]))
}

fn collide_sp_shape(sphere: CollisionSphere, shape: CollisionShape) -> Option<ContactManifold> {
    let c = sphere.center;

    let (closest, inside_normal, inside_depth) = match shape {
        CollisionShape::Point(p) => (p, None, 0.0),
        CollisionShape::Line(line) => {
            if line.bounds != LineBoundType::SEGMENT {
                // todo: sphere vs unbounded line contacts
                return None;
            }
            (closest_pt_segment(c, line.start, line.end), None, 0.0)
        }
        CollisionShape::Triangle(tri) => (closest_pt_triangle(c, tri), None, 0.0),
        CollisionShape::Box(bx) => {
            let axes = box_axes(&bx);
            let local = c - bx.center;
            let local = Vec3::new(local.dot(axes[0]), local.dot(axes[1]), local.dot(axes[2]));
            let clamped = local.zip_copied(bx.extents, |v, e| v.clamp(-e, e));

            if clamped == local {
                // center inside the box: push out through the nearest face
                let mut best = (f32::MAX, 0, 1.0);
                for i in 0..3 {
                    for sign in [-1.0, 1.0] {
                        let dist = bx.extents[i] - local[i] * sign;
                        if dist < best.0 {
                            best = (dist, i, sign);
                        }
                    }
                }
                let face_normal = axes[best.1] * best.2;
                (c + face_normal * best.0, Some(-face_normal), best.0)
            } else {
                let world = bx.center + axes[0] * clamped[0] + axes[1] * clamped[1] + axes[2] * clamped[2];
                (world, None, 0.0)
            }
        }
        CollisionShape::Sphere(other) => return collide_sp_sp(sphere, other),
        CollisionShape::Aabb(_) => return collide_sp_shape(sphere, normalize_shape(shape)),
    };

    if let Some(normal) = inside_normal {
        let depth = inside_depth + sphere.radius;
        return Some(ContactManifold::new(
            normal,
            &[ContactPoint {
                position: closest,
                depth,
            }],
        ));
    }

    let d = closest - c;
    let dist = len(d);
    let depth = sphere.radius - dist;
    if depth < 0.0 {
        return None;
    }

    let normal = if dist > EPS {
        d / dist
    } else if let CollisionShape::Triangle(tri) = shape {
        -triangle_normal(tri).unwrap_or(Vec3::new(0.0, 1.0, 0.0))
    } else {
        Vec3::new(0.0, -1.0, 0.0)
    };

    Some(ContactManifold::new(
        normal,
        &[ContactPoint {
            position: closest,
            depth,
        }],
    ))
}

// convex hulls (box, triangle, segment, point)

const EPS: f32 = 1e-6;

#[derive(Clone, Copy)]
struct HullFace {
    normal: Vec3<f32>,
    verts: [u8; 4],
    count: u8,
}

#[derive(Clone, Copy)]
struct Hull {
    verts: [Vec3<f32>; 8],
    vert_count: u8,
    faces: [HullFace; 6],
    face_count: u8,
    edges: [(u8, u8); 12],
    edge_count: u8,
    edge_dirs: [Vec3<f32>; 3],
    edge_dir_count: u8,
}

impl Hull {
    fn empty() -> Self {
        Self {
            verts: [Vec3::splat(0.0); 8],
            vert_count: 0,
            faces: [HullFace {
                normal: Vec3::splat(0.0),
                verts: [0; 4],
                count: 0,
            }; 6],
            face_count: 0,
            edges: [(0, 0); 12],
            edge_count: 0,
            edge_dirs: [Vec3::splat(0.0); 3],
            edge_dir_count: 0,
        }
    }

    fn from_shape(shape: CollisionShape) -> Option<Self> {
        let mut hull = Self::empty();

        match shape {
            CollisionShape::Point(p) => {
                hull.verts[0] = p;
                hull.vert_count = 1;
            }
            CollisionShape::Line(line) => {
                if line.bounds != LineBoundType::SEGMENT {
                    // todo: contacts with unbounded lines
                    return None;
                }
                hull.verts[0] = line.start;
                hull.verts[1] = line.end;
                hull.vert_count = 2;
                hull.edges[0] = (0, 1);
                hull.edge_count = 1;
                hull.edge_dirs[0] = (line.end - line.start).normalized();
                hull.edge_dir_count = 1;
            }
            CollisionShape::Triangle(tri) => {
                let n = triangle_normal(tri)?;
                hull.verts[..3].copy_from_slice(&tri);
                hull.vert_count = 3;
                hull.faces[0] = HullFace {
                    normal: n,
                    verts: [0, 1, 2, 0],
                    count: 3,
                };
                hull.faces[1] = HullFace {
                    normal: -n,
                    verts: [0, 2, 1, 0],
                    count: 3,
                };
                hull.face_count = 2;
                hull.edges[..3].copy_from_slice(&[(0, 1), (1, 2), (2, 0)]);
                hull.edge_count = 3;
                for (i, &(a, b)) in hull.edges[..3].iter().enumerate() {
                    hull.edge_dirs[i] = (tri[b as usize] - tri[a as usize]).normalized();
                }
                hull.edge_dir_count = 3;
            }
            CollisionShape::Box(bx) => {
                let axes = box_axes(&bx);
                for i in 0..8 {
                    let s = |bit: usize| if i & bit == 0 { -1.0 } else { 1.0 };
                    hull.verts[i] = bx.center
                        + axes[0] * (bx.extents[0] * s(1))
                        + axes[1] * (bx.extents[1] * s(2))
                        + axes[2] * (bx.extents[2] * s(4));
                }
                hull.vert_count = 8;

                let faces: [(usize, f32, [u8; 4]); 6] = [
                    (0, 1.0, [1, 3, 7, 5]),
                    (0, -1.0, [0, 2, 6, 4]),
                    (1, 1.0, [2, 3, 7, 6]),
                    (1, -1.0, [0, 1, 5, 4]),
                    (2, 1.0, [4, 5, 7, 6]),
                    (2, -1.0, [0, 1, 3, 2]),
                ];
                for (i, (axis, sign, verts)) in faces.into_iter().enumerate() {
                    hull.faces[i] = HullFace {
                        normal: axes[axis] * sign,
                        verts,
                        count: 4,
                    };
                }
                hull.face_count = 6;

                let mut e = 0;
                for a in 0..8u8 {
                    for bit in [1u8, 2, 4] {
                        if a & bit == 0 {
                            hull.edges[e] = (a, a | bit);
                            e += 1;
                        }
                    }
                }
                hull.edge_count = e as u8;
                hull.edge_dirs = axes;
                hull.edge_dir_count = 3;
            }
            CollisionShape::Aabb(_) | CollisionShape::Sphere(_) => return None,
        }

        Some(hull)
    }

    fn verts(&self) -> &[Vec3<f32>] {
        &self.verts[..self.vert_count as usize]
    }

    fn faces(&self) -> &[HullFace] {
        &self.faces[..self.face_count as usize]
    }

    fn edges(&self) -> &[(u8, u8)] {
        &self.edges[..self.edge_count as usize]
    }

    fn edge_dirs(&self) -> &[Vec3<f32>] {
        &self.edge_dirs[..self.edge_dir_count as usize]
    }

    fn project(&self, axis: Vec3<f32>) -> (f32, f32) {
        self.verts().iter().fold((f32::MAX, f32::MIN), |(min, max), v| {
            let d = v.dot(axis);
            (min.min(d), max.max(d))
        })
    }

    fn face_polygon(&self, face: &HullFace) -> ([Vec3<f32>; 4], usize) {
        let mut poly = [Vec3::splat(0.0); 4];
        for i in 0..face.count as usize {
            poly[i] = self.verts[face.verts[i] as usize];
        }
        (poly, face.count as usize)
    }

    // the face whose normal is most aligned with `dir`, or the vertices when the hull has no faces
    fn incident_polygon(&self, dir: Vec3<f32>) -> ([Vec3<f32>; 4], usize) {
        let best = self
            .faces()
            .iter()
            .max_by(|a, b| a.normal.dot(dir).total_cmp(&b.normal.dot(dir)));

        match best {
            Some(face) => self.face_polygon(face),
            None => {
                let mut poly = [Vec3::splat(0.0); 4];
                let n = self.verts().len().min(4);
                poly[..n].copy_from_slice(&self.verts()[..n]);
                (poly, n)
            }
        }
    }
}

#[derive(Clone, Copy)]
enum Feature {
    FaceA(usize),
    FaceB(usize),
    Edges(Vec3<f32>, Vec3<f32>),
}

fn collide_hulls(a: Hull, b: Hull) -> Option<ContactManifold> {
    if a.face_count == 0 && b.face_count == 0 {
        // todo: point/segment vs point/segment contacts
        return None;
    }

    let mut best: Option<(f32, Vec3<f32>, Feature)> = None;

    let mut test_axis = |axis: Vec3<f32>, feature: Feature, bias: f32| -> bool {
        let (min_a, max_a) = a.project(axis);
        let (min_b, max_b) = b.project(axis);
        let forward = max_a - min_b;
        let backward = max_b - min_a;
        if forward < 0.0 || backward < 0.0 {
            return false;
        }
        let (depth, normal) = if forward <= backward {
            (forward, axis)
        } else {
            (backward, -axis)
        };
        if best.is_none_or(|(d, _, _)| depth * bias < d) {
            best = Some((depth, normal, feature));
        }
        true
    };

    for (i, face) in a.faces().iter().enumerate() {
        if !test_axis(face.normal, Feature::FaceA(i), 1.0) {
            return None;
        }
    }
    for (i, face) in b.faces().iter().enumerate() {
        if !test_axis(face.normal, Feature::FaceB(i), 1.0) {
            return None;
        }
    }
    for &da in a.edge_dirs() {
        for &db in b.edge_dirs() {
            let axis = da.cross(db);
            let l = len(axis);
            if l < 1e-4 {
                continue;
            }
            // edge axes must be clearly better than face axes to win, which keeps resting contacts stable
            if !test_axis(axis / l, Feature::Edges(da, db), 1.05) {
                return None;
            }
        }
    }

    let (depth, normal, feature) = best?;

    let feature = match feature {
        // a face axis whose direction got flipped is really the opposite face of that hull
        Feature::FaceA(i) if a.faces[i].normal.dot(normal) < 0.0 => {
            match a.faces().iter().position(|f| f.normal.dot(normal) > 0.999) {
                Some(j) => Feature::FaceA(j),
                None => Feature::Edges(Vec3::splat(0.0), Vec3::splat(0.0)),
            }
        }
        Feature::FaceB(i) if b.faces[i].normal.dot(normal) > 0.0 => {
            match b.faces().iter().position(|f| f.normal.dot(normal) < -0.999) {
                Some(j) => Feature::FaceB(j),
                None => Feature::Edges(Vec3::splat(0.0), Vec3::splat(0.0)),
            }
        }
        f => f,
    };

    let mut points = [ContactPoint::default(); 8];
    let mut count = 0;

    match feature {
        Feature::FaceA(i) => {
            let (reference, ref_count) = a.face_polygon(&a.faces[i]);
            let (incident, inc_count) = b.incident_polygon(-normal);
            for p in clip_polygon(&incident[..inc_count], &reference[..ref_count], normal) {
                let separation = (p - reference[0]).dot(normal);
                if separation <= 0.0 && count < points.len() {
                    points[count] = ContactPoint {
                        position: p - normal * (separation * 0.5),
                        depth: -separation,
                    };
                    count += 1;
                }
            }
        }
        Feature::FaceB(i) => {
            let (reference, ref_count) = b.face_polygon(&b.faces[i]);
            let (incident, inc_count) = a.incident_polygon(normal);
            for p in clip_polygon(&incident[..inc_count], &reference[..ref_count], -normal) {
                let separation = (p - reference[0]).dot(-normal);
                if separation <= 0.0 && count < points.len() {
                    points[count] = ContactPoint {
                        position: p + normal * (separation * 0.5),
                        depth: -separation,
                    };
                    count += 1;
                }
            }
        }
        Feature::Edges(da, db) => {
            let edge_a = support_edge(&a, da, normal);
            let edge_b = support_edge(&b, db, -normal);
            if let (Some((a0, a1)), Some((b0, b1))) = (edge_a, edge_b) {
                let (ca, cb) = closest_pts_segments(a0, a1, b0, b1);
                points[0] = ContactPoint {
                    position: (ca + cb) * 0.5,
                    depth,
                };
                count = 1;
            }
        }
    }

    if count == 0 {
        // clipping lost every point (grazing contact): fall back to the deepest vertex of b
        let p = b
            .verts()
            .iter()
            .copied()
            .min_by(|x, y| x.dot(normal).total_cmp(&y.dot(normal)))?;
        points[0] = ContactPoint {
            position: p + normal * (depth * 0.5),
            depth,
        };
        count = 1;
    }

    Some(ContactManifold::new(normal, &points[..count]))
}

fn support_edge(hull: &Hull, dir: Vec3<f32>, normal: Vec3<f32>) -> Option<(Vec3<f32>, Vec3<f32>)> {
    if dir == Vec3::splat(0.0) {
        return None;
    }

    hull.edges()
        .iter()
        .map(|&(i, j)| (hull.verts[i as usize], hull.verts[j as usize]))
        .filter(|&(p, q)| (q - p).normalized().dot(dir).abs() > 0.999)
        .max_by(|x, y| ((x.0 + x.1).dot(normal)).total_cmp(&(y.0 + y.1).dot(normal)))
}

// clips `incident` against the side planes of the `reference` polygon (whose outward normal is `normal`)
fn clip_polygon(incident: &[Vec3<f32>], reference: &[Vec3<f32>], normal: Vec3<f32>) -> Vec<Vec3<f32>> {
    let mut poly = incident.to_vec();
    let centroid = reference.iter().fold(Vec3::splat(0.0), |s, &p| s + p) / reference.len() as f32;

    for i in 0..reference.len() {
        let p0 = reference[i];
        let p1 = reference[(i + 1) % reference.len()];
        let mut side = (p1 - p0).cross(normal).normalized();
        if (centroid - p0).dot(side) > 0.0 {
            side = -side;
        }

        poly = clip_by_plane(&poly, p0, side);
        if poly.is_empty() {
            break;
        }
    }

    poly
}

// keeps the part of `poly` behind the plane (dot(p - origin, normal) <= 0)
fn clip_by_plane(poly: &[Vec3<f32>], origin: Vec3<f32>, normal: Vec3<f32>) -> Vec<Vec3<f32>> {
    let dist = |p: Vec3<f32>| (p - origin).dot(normal);

    match poly.len() {
        0 => vec![],
        1 => {
            if dist(poly[0]) <= 0.0 {
                poly.to_vec()
            } else {
                vec![]
            }
        }
        2 => {
            let (p, q) = (poly[0], poly[1]);
            let (dp, dq) = (dist(p), dist(q));
            match (dp <= 0.0, dq <= 0.0) {
                (true, true) => vec![p, q],
                (false, false) => vec![],
                _ => {
                    let x = p + (q - p) * (dp / (dp - dq));
                    if dp <= 0.0 { vec![p, x] } else { vec![x, q] }
                }
            }
        }
        _ => {
            let mut out = Vec::with_capacity(poly.len() + 2);
            for i in 0..poly.len() {
                let p = poly[i];
                let q = poly[(i + 1) % poly.len()];
                let (dp, dq) = (dist(p), dist(q));
                if dp <= 0.0 {
                    out.push(p);
                }
                if (dp <= 0.0) != (dq <= 0.0) {
                    out.push(p + (q - p) * (dp / (dp - dq)));
                }
            }
            out
        }
    }
}

fn reduce_points(points: &[ContactPoint]) -> Vec<ContactPoint> {
    if points.len() <= ContactManifold::MAX_POINTS {
        return points.to_vec();
    }

    let mut rest = points.to_vec();
    let mut chosen = Vec::with_capacity(ContactManifold::MAX_POINTS);

    let deepest = (0..rest.len())
        .max_by(|&i, &j| rest[i].depth.total_cmp(&rest[j].depth))
        .unwrap();
    chosen.push(rest.swap_remove(deepest));

    while chosen.len() < ContactManifold::MAX_POINTS && !rest.is_empty() {
        let score = |p: &ContactPoint| {
            chosen
                .iter()
                .map(|c: &ContactPoint| (c.position - p.position).length_sq())
                .fold(f32::MAX, f32::min)
        };
        let farthest = (0..rest.len())
            .max_by(|&i, &j| score(&rest[i]).total_cmp(&score(&rest[j])))
            .unwrap();
        chosen.push(rest.swap_remove(farthest));
    }

    chosen
}

// geometry helpers

pub(crate) fn len(v: Vec3<f32>) -> f32 {
    v.length_sq().sqrt()
}

pub(crate) fn box_axes(bx: &CollisionBox) -> [Vec3<f32>; 3] {
    let m = bx.rotation.to_matrix();
    [
        m * Vec3::new(1.0, 0.0, 0.0),
        m * Vec3::new(0.0, 1.0, 0.0),
        m * Vec3::new(0.0, 0.0, 1.0),
    ]
}

pub(crate) fn triangle_normal(tri: [Vec3<f32>; 3]) -> Option<Vec3<f32>> {
    let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]);
    let l = len(n);
    (l > EPS).then(|| n / l)
}

fn closest_pt_segment(p: Vec3<f32>, a: Vec3<f32>, b: Vec3<f32>) -> Vec3<f32> {
    let ab = b - a;
    let l = ab.length_sq();
    if l <= EPS {
        return a;
    }
    a + ab * ((p - a).dot(ab) / l).clamp(0.0, 1.0)
}

fn closest_pts_segments(p1: Vec3<f32>, q1: Vec3<f32>, p2: Vec3<f32>, q2: Vec3<f32>) -> (Vec3<f32>, Vec3<f32>) {
    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;
    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);

    let (s, t) = if a <= EPS && e <= EPS {
        (0.0, 0.0)
    } else if a <= EPS {
        (0.0, (f / e).clamp(0.0, 1.0))
    } else {
        let c = d1.dot(r);
        if e <= EPS {
            ((-c / a).clamp(0.0, 1.0), 0.0)
        } else {
            let b = d1.dot(d2);
            let denom = a * e - b * b;
            let mut s = if denom > EPS {
                ((b * f - c * e) / denom).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let mut t = (b * s + f) / e;
            if t < 0.0 {
                t = 0.0;
                s = (-c / a).clamp(0.0, 1.0);
            } else if t > 1.0 {
                t = 1.0;
                s = ((b - c) / a).clamp(0.0, 1.0);
            }
            (s, t)
        }
    };

    (p1 + d1 * s, p2 + d2 * t)
}

pub(crate) fn closest_pt_triangle(p: Vec3<f32>, [a, b, c]: [Vec3<f32>; 3]) -> Vec3<f32> {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }

    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }

    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }

    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }

    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }

    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }

    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

// raycasts

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Debug)]
pub struct RayHit {
    pub t: f32,
    pub point: Vec3<f32>,
    pub normal: Vec3<f32>,
}

pub fn raycast(line: CollisionLine, shape: CollisionShape) -> Option<RayHit> {
    let dir = line.end - line.start;
    let t_min = if line.bounds.is_start_restricted() {
        0.0
    } else {
        f32::NEG_INFINITY
    };
    let t_max = if line.bounds.is_end_restricted() { 1.0 } else { f32::INFINITY };

    let pick = |entry: Option<(f32, Vec3<f32>)>, exit: Option<(f32, Vec3<f32>)>| {
        [entry, exit]
            .into_iter()
            .flatten()
            .find(|&(t, _)| t >= t_min && t <= t_max)
            .map(|(t, normal)| RayHit {
                t,
                point: line.start + dir * t,
                normal,
            })
    };

    match shape {
        CollisionShape::Sphere(sp) => {
            let m = line.start - sp.center;
            let a = dir.dot(dir);
            if a <= EPS {
                return None;
            }
            let b = m.dot(dir);
            let c = m.dot(m) - sp.radius * sp.radius;
            let disc = b * b - a * c;
            if disc < 0.0 {
                return None;
            }
            let sq = disc.sqrt();
            let normal_at = |t: f32| (line.start + dir * t - sp.center).normalized();
            let t0 = (-b - sq) / a;
            let t1 = (-b + sq) / a;
            pick(Some((t0, normal_at(t0))), Some((t1, normal_at(t1))))
        }
        CollisionShape::Aabb(aabb) => raycast_box(line.start, dir, aabb, [Vec3::X, Vec3::Y, Vec3::Z], pick),
        CollisionShape::Box(bx) => raycast_box(
            line.start,
            dir,
            CollisionAabb {
                center: bx.center,
                extents: bx.extents,
            },
            box_axes(&bx),
            pick,
        ),
        CollisionShape::Triangle(tri) => {
            let n = triangle_normal(tri)?;
            let e1 = tri[1] - tri[0];
            let e2 = tri[2] - tri[0];
            let p = dir.cross(e2);
            let det = e1.dot(p);
            if det.abs() <= EPS {
                return None;
            }
            let inv = 1.0 / det;
            let s = line.start - tri[0];
            let u = s.dot(p) * inv;
            if !(0.0..=1.0).contains(&u) {
                return None;
            }
            let q = s.cross(e1);
            let v = dir.dot(q) * inv;
            if v < 0.0 || u + v > 1.0 {
                return None;
            }
            let t = e2.dot(q) * inv;
            let normal = if n.dot(dir) > 0.0 { -n } else { n };
            pick(Some((t, normal)), None)
        }
        CollisionShape::Point(_) | CollisionShape::Line(_) => None,
    }
}

fn raycast_box(
    origin: Vec3<f32>,
    dir: Vec3<f32>,
    bx: CollisionAabb,
    axes: [Vec3<f32>; 3],
    pick: impl Fn(Option<(f32, Vec3<f32>)>, Option<(f32, Vec3<f32>)>) -> Option<RayHit>,
) -> Option<RayHit> {
    let rel = origin - bx.center;
    let mut entry = (f32::NEG_INFINITY, Vec3::splat(0.0));
    let mut exit = (f32::INFINITY, Vec3::splat(0.0));

    for i in 0..3 {
        let o = rel.dot(axes[i]);
        let d = dir.dot(axes[i]);
        let e = bx.extents[i];
        if d.abs() <= EPS {
            if o.abs() > e {
                return None;
            }
            continue;
        }
        let (mut t0, mut t1) = ((-e - o) / d, (e - o) / d);
        let mut n0 = -axes[i];
        let mut n1 = axes[i];
        if t0 > t1 {
            std::mem::swap(&mut t0, &mut t1);
            std::mem::swap(&mut n0, &mut n1);
        }
        if t0 > entry.0 {
            entry = (t0, n0);
        }
        if t1 < exit.0 {
            exit = (t1, n1);
        }
        if entry.0 > exit.0 {
            return None;
        }
    }

    pick(Some(entry), Some(exit))
}
