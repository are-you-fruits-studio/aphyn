use std::path::Path;

use fruits_asset_storage::AssetStorageResource;
use fruits_ecs::{ResourcesHolderMut, ResourcesHolderRef};
use fruits_ffi::FfiFnMutMut;
use fruits_math::{Vec2, Vec3, Vec4};
use fruits_render_core::{CoordinateSpaceType, RenderApiResource, StandardMesh, StandardMeshAssetMetadata, StandardVertex};
use fruits_serialization::*;

use crate::AssetLoader;

pub struct MeshHandleLoader<'a> {
    pub render_api: &'a RenderApiResource,
    pub meshes: &'a mut AssetStorageResource<StandardMesh>,
}

impl<'a> MeshHandleLoader<'a> {
    pub fn from_world(res: ResourcesHolderMut<'a>) -> Option<Self> {
        Some(unsafe { Self {
            render_api: &*res.get_ptr::<RenderApiResource>()?,
            meshes: &mut *res.get_ptr::<AssetStorageResource<StandardMesh>>()?,
        }})
    }
}
impl<'a, S: Copy + SerializerCtxState<StandardMeshAssetMetadata>> AssetLoader<S> for MeshHandleLoader<'a> {
    type Asset = StandardMesh;
    type SelfWithAnotherLifetime<'r> = MeshHandleLoader<'r>;

    fn create_loader<'r>(res: ResourcesHolderMut<'r>) -> Option<Self::SelfWithAnotherLifetime<'r>> {
        Self::SelfWithAnotherLifetime::from_world(res)
    }
    
    fn get_related_asset_storage(&mut self) -> &mut AssetStorageResource<Self::Asset> {
        self.meshes
    }
    
    fn load_from_serialized(&mut self, mut ctx: SerializerCtx<S>, value: &SerializedValue, assets_dir_path: impl AsRef<Path>) -> Option<Self::Asset> {
        MeshLoader {
            render_api: self.render_api,
        }.load_from_deserialized(ctx.deserialize_default("", value), assets_dir_path)
    }
}

//

pub struct MeshLoader<'a> {
    pub render_api: &'a RenderApiResource,
}

impl<'a> MeshLoader<'a> {
    pub fn from_world(res: ResourcesHolderRef<'a>) -> Option<Self> {
        Some(Self {
            render_api: res.get::<RenderApiResource>()?,
        })
    }
    
    pub fn load_from_serialized(&mut self, value: &SerializedValue, assets_dir_path: impl AsRef<Path>) -> Option<StandardMesh> {
        let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
        let mut ctx = SerializerCtx::new(PureSerializerCtxState, &mut err_handler);
        let value = ctx.deserialize_default::<StandardMeshAssetMetadata>("", value);

        self.load_from_deserialized(value, assets_dir_path)
    }
    pub fn load_from_deserialized(&mut self, value: StandardMeshAssetMetadata, assets_dir_path: impl AsRef<Path>) -> Option<StandardMesh> {
        let mut path = assets_dir_path.as_ref().to_path_buf();
        path.push(value.raw_mesh.as_str());
    
        let mesh_data = match std::fs::read_to_string(path) {
            Ok(data) => data,
            Err(_err) => return None,
        };
    
        let obj = fruits_wavefront::parse_obj(&mesh_data)?;
    
        let mut vertices = Vec::new();
    
        for face in obj.faces {
            let face = face.map(|vertex| Some(StandardVertex {
                position: obj.positions.get(vertex.v).copied()?,
                color: [1.0; 4],
                normal: vertex.vn.map(|n| obj.normals.get(n).copied()).flatten()?,
                // tangent is calculated later inside this function
                tangent: [0.0; 4],
                uv: vertex.vt.map(|n| obj.texcoords.get(n).copied()).flatten()?,
            }));
            if face.iter().any(|f| f.is_none()) {
                return None;
            }
            let mut face = face.map(|o| o.unwrap());

            let (tangent, bitangent) = calculate_tangent_bitangent(
                face.map(|f| Vec3::from_array(f.position)),
                face.map(|f| Vec2::from_array(f.uv)),
            );

            for vertex in &mut face {
                vertex.tangent = calculate_tangent_handedness_by_orthogonalizing(
                    Vec3::from_array(vertex.normal),
                    tangent,
                    bitangent,
                ).into_array();
            }

            for vertex in face {
                vertices.push(vertex);
            }
        }
    
        if value.coordinate_space == CoordinateSpaceType::RightHandZBack {
            for vertex in &mut vertices {
                vertex.position[2] *= -1.0;
                vertex.normal[2] *= -1.0;
            }
        
        } else if value.coordinate_space == CoordinateSpaceType::RightHandZUp {
            for vertex in &mut vertices {
                (vertex.position[1], vertex.position[2]) = (vertex.position[2], vertex.position[1]);
                (vertex.normal[1], vertex.normal[2]) = (vertex.normal[2], vertex.normal[1]);
            }
        }
    
        let mut indices = (0..vertices.len()).map(|i| i as u16).collect::<Vec<_>>();
    
        if value.has_clockwise_winding {
            if value.coordinate_space == CoordinateSpaceType::RightHandZBack || value.coordinate_space == CoordinateSpaceType::RightHandZUp {
                for i in 0..(indices.len() / 3) {
                    let offset = i * 3;
                    (indices[offset + 1], indices[offset + 2]) = (indices[offset + 2], indices[offset + 1])
                }
            }
        }
    
        if value.has_inverted_u {
            for vertex in &mut vertices {
                vertex.uv[0] = 1.0 - vertex.uv[0];
            }
        }
        if value.has_inverted_v {
            for vertex in &mut vertices {
                vertex.uv[1] = 1.0 - vertex.uv[1];
            }
        }
    
        let mesh = self.render_api.create_mesh(&vertices, &indices, Some(value));
    
        Some(mesh)
    }
}

pub fn calculate_tangent_handedness_by_orthogonalizing(
    normal: Vec3<f32>,
    tangent: Vec3<f32>,
    bitangent: Vec3<f32>,
) -> Vec4<f32> {
    let tangent = tangent.normalized();

    let tangent = (tangent - normal * normal.dot(tangent)).normalized();

    let handedness = if normal.cross(tangent).dot(bitangent) < 0.0 {
        -1.0
    } else {
        1.0
    };

    tangent.xyzn(handedness)
}

pub fn calculate_tangent_bitangent(
    pos: [Vec3<f32>; 3],
    uv: [Vec2<f32>; 3],
) -> (Vec3<f32>, Vec3<f32>) {
    let edge1 = pos[1] - pos[0];
    let edge2 = pos[2] - pos[0];

    let duv1 = uv[1] - uv[0];
    let duv2 = uv[2] - uv[0];

    let r = 1.0 / (duv1.x * duv2.y - duv1.y * duv2.x);

    let tangent = (edge1 * duv2.y - edge2 * duv1.y) * r;

    let bitangent = (edge2 * duv1.x - edge1 * duv2.x) * r;

    (tangent.normalized(), bitangent.normalized())
}