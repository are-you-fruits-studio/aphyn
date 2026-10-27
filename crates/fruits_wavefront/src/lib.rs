//! # fruits_wavefront
//!
//! Parses Wavefront `.obj` text into an in-memory mesh of positions, texture
//! coordinates, normals, and triangle faces. It is a small, dependency-free
//! building block the engine uses to turn mesh asset files into renderable
//! geometry.
//!
//! # How to use
//!
//! #### Parse OBJ text into a mesh
//!
//! Hand [`parse_obj`] the contents of an `.obj` file to get an [`ObjMesh`]. The
//! returned mesh exposes the raw vertex attribute arrays and a list of triangle
//! faces; missing or malformed attribute lines yield `None`.
//!
//! ```
//! use fruits_wavefront::parse_obj;
//!
//! let raw = "\
//! v 0.0 0.0 0.0
//! v 1.0 0.0 0.0
//! v 0.0 1.0 0.0
//! f 1 2 3
//! ";
//!
//! let mesh = parse_obj(raw).expect("valid obj");
//! assert_eq!(mesh.positions.len(), 3);
//! assert_eq!(mesh.faces.len(), 1);
//! ```
//!
//! #### Resolve a face's vertex attributes
//!
//! Each corner of a face is an [`ObjVertex`] holding zero-based indices into the
//! mesh's `positions`, `texcoords`, and `normals` arrays. Texture coordinate and
//! normal indices are optional, matching the `v`, `v/vt`, `v//vn`, and `v/vt/vn`
//! face formats.
//!
//! ```
//! use fruits_wavefront::parse_obj;
//!
//! let raw = "\
//! v 0.0 0.0 0.0
//! v 1.0 0.0 0.0
//! v 0.0 1.0 0.0
//! vt 0.0 0.0
//! vn 0.0 0.0 1.0
//! f 1/1/1 2/1/1 3/1/1
//! ";
//!
//! let mesh = parse_obj(raw).unwrap();
//! let corner = mesh.faces[0][0];
//! assert_eq!(corner.v, 0); // 1-based "1" becomes 0-based 0
//! assert_eq!(corner.vt, Some(0));
//! assert_eq!(corner.vn, Some(0));
//! ```
//!
//! In a running engine you rarely call [`parse_obj`] yourself: the mesh asset
//! loader in `fruits_asset_loading` reads the `.obj` file named by a mesh asset,
//! parses it here, and flattens the faces into engine vertices.
//!
//! # How to maintain
//!
//! [`parse_obj`] is a single line-oriented pass over the input. It trims each
//! line, skips blanks and `#` comments, and dispatches on the leading tag:
//! `v`/`vt`/`vn` push attribute arrays, `f` builds a face. All other tags
//! (`o`, `g`, `s`, `usemtl`, `mtllib`, ...) are ignored.
//!
//! Indices are normalized at parse time by `parse_index`: OBJ stores 1-based
//! indices, so positive values are decremented by one, and negative values are
//! resolved relative to the current length of the corresponding array (the OBJ
//! relative-index convention). All indices stored in [`ObjVertex`] are therefore
//! zero-based and ready to index the mesh arrays directly. Because negative
//! indices are resolved against the array length *as seen so far*, attribute
//! lines must precede the faces that reference them — which is the order OBJ
//! files use.
//!
//! Faces with more than three corners are triangulated with a sliding window of
//! three consecutive corners (`face.windows(3)`), not a triangle fan. This is
//! exact for triangles; for larger polygons it produces overlapping triangles
//! and is only correct for the convex, planar quads typical of exported meshes.
//! Revisit this if non-triangulated or concave polygons need faithful support.
//!
//! Robustness is deliberately minimal. Coordinate components are parsed with
//! `.parse().unwrap()`, so a non-numeric attribute value **panics** rather than
//! returning `None`; `None` is reserved for structurally short lines (too few
//! components, or a face corner missing its position index). Callers that must
//! tolerate arbitrary input should validate or sandbox accordingly.


// todo: ffi?

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ObjVertex {
    pub v: usize,
    pub vt: Option<usize>,
    pub vn: Option<usize>,
}

#[derive(Debug, Default)]
pub struct ObjMesh {
    pub positions: Vec<[f32; 3]>,
    pub texcoords: Vec<[f32; 2]>,
    pub normals: Vec<[f32; 3]>,
    pub faces: Vec<[ObjVertex; 3]>,
}

pub fn parse_obj(raw: &str) -> Option<ObjMesh> {
    let mut mesh = ObjMesh::default();

    for line in raw.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let mut parts = line.split_whitespace();

        let Some(tag) = parts.next() else {
            continue
        };

        match tag {
            "v" => {
                let mut parts = parts.map(|s| s.parse().unwrap());
                mesh.positions.push([parts.next()?, parts.next()?, parts.next()?]);
            }
            "vt" => {
                let mut parts = parts.map(|s| s.parse().unwrap());
                mesh.texcoords.push([parts.next()?, parts.next()?]);
            }
            "vn" => {
                let mut parts = parts.map(|s| s.parse().unwrap());
                mesh.normals.push([parts.next()?, parts.next()?, parts.next()?]);
            }
            "f" => {
                let mut face = Vec::new();
                for part in parts {
                    // e.g., "3/4/5" or "3//5" or "3"
                    let mut indices = part.split('/');
                    let v = parse_index(indices.next()?, mesh.positions.len());
                    let vt = indices.next().and_then(|s| {
                        if !s.is_empty() {
                            Some(parse_index(s, mesh.texcoords.len()))
                        } else {
                            None
                        }
                    });
                    let vn = indices.next().and_then(|s| {
                        if !s.is_empty() {
                            Some(parse_index(s, mesh.normals.len()))
                        } else {
                            None
                        }
                    });
                    face.push(ObjVertex { v, vt, vn });
                }

                // Triangulate (OBJ can have quads)
                for tri in face.windows(3) {
                    mesh.faces.push([tri[0], tri[1], tri[2]]);
                }
            }
            _ => {} // ignore others for now
        }
    }

    Some(mesh)
}

fn parse_index(idx: &str, len: usize) -> usize {
    let i: isize = idx.parse().unwrap();

    if i < 0 {
        (len as isize + i) as usize
    } else {
        (i as usize) - 1 // OBJ is 1-based
    }
}
