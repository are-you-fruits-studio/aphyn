//! # fruits_asset_loading
//!
//! Loads engine assets — textures, materials, meshes, audio clips, and prefabs — from asset
//! files on disk into the world's asset storages, registered under a key so the rest of the
//! engine can reference them by name, and instantiates or records prefabs.
//!
//! # How to use
//!
//! Every asset is identified by a **key**: the `/`-separated path, relative to the `assets/`
//! directory, of a small JSON *asset file* with the `.asset` extension. The file declares an
//! `asset_type` (`"texture"`, `"material"`, `"mesh"`, `"audio_clip"`, `"prefab"`, or `"font"`)
//! and the asset's settings, and usually points at a raw payload (image, `.obj`, `.wav`) by a
//! further `assets/`-relative path. The module is part of the engine's default modules
//! (`add_defult_modules_to`), which also registers the texture, material, mesh, and audio-clip
//! storages it fills.
//!
//! #### Using a loaded asset
//!
//! During the [`Start`](fruits_ecs::Schedule::Start) pass the module loads every `.asset` file
//! under `assets/`. Order your setup system after [`SYSTEM_GROUP_ASSETS`], then look the asset
//! up by key in its storage:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! fn main() {
//!     let mut app = App::new();
//!     add_defult_modules_to(app.ecs_mut().as_mut());
//!
//!     let mut behavior = app.ecs_mut().behavior_mut();
//!     let mut start = behavior.get_mut(Schedule::Start);
//!     start.insert_system(setup_scene);
//!     start.order_group(SYSTEM_GROUP_ASSETS).before_system(setup_scene);
//!
//!     app.run();
//! }
//!
//! fn setup_scene(mut world: WorldDataMut) {
//!     let (res, mut ent, _evt) = world.as_tuple_mut();
//!
//!     let meshes = res.get::<AssetStorageResource<StandardMesh>>().unwrap();
//!     let cube_mesh = meshes.get_registered("meshes/cube.asset").unwrap().clone();
//!     // store `cube_mesh` on a component ...
//! }
//! ```
//!
//! #### Writing an asset file
//!
//! The fields besides `asset_type` are the serialized form of the matching metadata type
//! (`StandardTextureAssetMetadata`, `StandardMaterialAssetMetadata`,
//! `StandardMeshAssetMetadata`, `AudioClipAssetMetadata`); fields left out keep their defaults.
//! An asset handle inside an asset file is written as the referenced asset's key and is loaded
//! along with it:
//!
//! ```text
//! // assets/textures/player.asset
//! { "asset_type": "texture", "raw_texture": "textures/player.png" }
//!
//! // assets/materials/player.asset
//! { "asset_type": "material", "is_lit": true, "color_tex": "textures/player.asset" }
//! ```
//!
//! #### Instantiating a prefab
//!
//! [`instantiate_prefab`] spawns a loaded prefab's entities and components into the world and
//! returns the root entity:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! fn spawn_enemy(mut world: WorldDataMut) {
//!     let (res, mut ent, _evt) = world.as_tuple_mut();
//!
//!     let prefab = res.get::<AssetStorageResource<Prefab>>().unwrap()
//!         .get_registered("prefabs/enemy.asset").unwrap().clone();
//!
//!     let root = instantiate_prefab(res.as_ref(), ent.as_mut(), prefab).unwrap();
//! }
//! ```
//!
//! #### Recording an entity hierarchy into a prefab
//!
//! [`record_into_prefab`] captures an entity and all of its descendants (through
//! `ParentComponent`) as a [`Prefab`]:
//!
//! ```ignore
//! use fruits_engine::*;
//!
//! let prefab = record_into_prefab(res.as_ref(), ent.as_ref(), root).unwrap();
//! ```
//!
//! # How to maintain
//!
//! #### Startup loading
//!
//! [`add_asset_module_to`] inserts the `AssetStorageResource<Prefab>` and registers
//! [`load_all_assets_system`] in the `Start` schedule under [`SYSTEM_GROUP_ASSETS`]. It calls
//! [`load_all_assets`] with the `assets` directory relative to the working directory, which
//! walks it recursively and, for every `.asset` file, reads the JSON, validates `asset_type`
//! ([`AssetType`]), builds the key from the path, and deserializes that key *as an
//! `AssetHandle<T>`* of the matching type. Failures are printed and the file is skipped; a
//! `"font"` asset hits a `todo!()` and panics.
//!
//! #### Loading is driven by the serializers
//!
//! The deserialization above runs through a context made of the world's `SerializersResource`
//! wrapped with a local registry built by [`load_asset_transitively`]. That registry holds an
//! [`EntityTransSerializer`] and a [`TransitiveLoadTransSerializer`] per asset type. Its
//! `deserialize` turns a key into a handle: an empty key yields an empty handle; a key that is
//! registered and still resolves returns the cached handle; a registered key whose asset was
//! removed is unregistered; otherwise it reads `assets/<key>`, converts the JSON to a
//! `SerializedValue`, runs the type's loader, and stores the result with `insert_and_register`.
//! Because the loaders deserialize the asset file's metadata through the same context, any
//! `AssetHandle` field inside it (for example a material's textures) is loaded transitively,
//! and loading the same key again returns the same handle. The asset storages are wrapped in
//! [`std::sync::Mutex`] because several serializers borrow them at once.
//!
//! [`save_with_asset_serializers`] builds the counterpart registry for saving:
//! [`AssetHandleLinkTransSerializer`] writes a handle as its registered key, and
//! [`DirectAssetSaveTransSerializer`] writes a [`DirectSerializableAsset`] as the asset's own
//! metadata (or the prefab's entities); its `deserialize` is a `todo!()`.
//! [`load_asset_single`] loads one asset's content under a given key
//! ([`SingleDirectLoadTransSerializer`]) while resolving other handles only from what is already
//! registered ([`SingleLoadTransSerializer`]). Each of these has a `*_from_world` variant that
//! fetches the resources out of [`fruits_ecs::ResourcesHolderMut`] through raw pointers.
//!
//! #### Loaders
//!
//! - **Texture** — [`TextureLoader`] decodes `raw_texture` with the `image` crate into RGBA8
//!   and creates it through `RenderApiResource::create_texture` with `FilterMode::Nearest`,
//!   passing the metadata along.
//! - **Material** — [`MaterialLoader`] creates the material from its metadata, looking the
//!   color, roughness, metallic, normal, and emission texture handles up in the texture storage.
//! - **Mesh** — [`MeshLoader`] parses `raw_mesh` as Wavefront `.obj` with `fruits_wavefront`.
//!   Faces are flattened into a non-indexed vertex list (a face missing a position, normal, or
//!   UV fails the load), each face gets a tangent with handedness, and the vertices are fixed
//!   up per `coordinate_space` (`RightHandZBack` negates Z, `RightHandZUp` swaps Y and Z).
//!   `has_clockwise_winding` swaps triangle order only for the right-handed spaces, and
//!   `has_inverted_u` / `has_inverted_v` flip the UVs. Indices are `u16`.
//! - **Audio clip** — [`AudioClipLoader`] reads the `raw_audio` `.wav` with `hound`. Integer
//!   samples are normalized to `f32`, the buffer is forced to stereo, and it is resampled to
//!   the engine's sample rate when the file's rate differs.
//! - **Prefab** — [`PrefabLoader`] deserializes the file (an `entities` list of `entity_id` +
//!   `components`) with a pure context, then deserializes every component through the loading
//!   context so the assets it references get loaded.
//!
//! The `*HandleLoader` types and the [`AssetLoader`] trait wrap these loaders with a
//! cache-or-load-from-key path ([`AssetLoader::get_or_load_from_key`]) that is not used by the
//! startup loading.
//!
//! #### Prefab instantiation and recording
//!
//! [`instantiate_prefab`] creates one entity per prefab-local id (the first id is the root),
//! then deserializes every component through a context that layers a local registry over the
//! world's `SerializersResource`: [`EntityTransSerializer`] maps stored ids to the new entities
//! (`0` is the empty entity), and [`PrefabAssetInstantiateTransSerializer`] resolves asset
//! keys from the prefab's `dependencies`. The prefab loader does not fill those dependencies
//! yet (`load_prefab_dependencies` returns an empty set), so asset handles inside an
//! instantiated prefab currently fail to resolve and report an error.
//! [`record_into_prefab`] walks the hierarchy breadth-first through `ParentComponent`, gives
//! each entity the id `index + 1`, and serializes its components with entity references
//! mapped to those ids; [`override_entity_components_from_prefab`] removes all components of
//! an entity and deserializes the given ones onto it.
//! [`deserialize_entity_component_from_prefab`] writes one component at a path with the same
//! id mapping, adding the component when the entity doesn't have it yet.
//!
//! The crate root keeps `todo` notes on the asset formats still to be supported (font import,
//! and `.obj`/`.fbx` import details), and the `_*_FILE_EXAMPLE` constants are older format
//! sketches that do not match the current metadata fields.

mod material;
mod mesh;
mod texture;
mod prefab;
mod audio_clip;
mod serializers;

use std::{ffi::OsStr, path::{Path, PathBuf}};

use fruits_asset_storage::{AssetHandle, AssetStorageResource};
use fruits_audio::AudioClip;
use fruits_prefab::Prefab;
use fruits_render_core::{StandardMaterial, StandardMesh, StandardTexture};
use fruits_serialization::{SerializedPrimitive, SerializedValue, SerializerCtx};
pub use material::*;
pub use mesh::*;
pub use texture::*;
pub use prefab::*;
pub use audio_clip::*;
pub use serializers::*;

use fruits_ecs::{ResourcesHolderMut, Schedule, WorldBuilderMut};

// todo: specify supported file formats.

// todo: asset types:
// - mesh (import details of the existing: .obj, .fbx)
// + texture (import details of the existing: .bmp, .png, .jpg)
// + material
// - font
// +/2 prefab
// + audio_clip

pub const SYSTEM_GROUP_ASSETS: &'static str = "fruits_assets";

pub fn add_asset_module_to(mut world: WorldBuilderMut) {
    world.data_mut().resources_mut().insert(AssetStorageResource::<Prefab>::new());

    world.behavior_mut()
        .get_mut(Schedule::Start)
        .group(SYSTEM_GROUP_ASSETS)
        .insert_child_system(load_all_assets_system);
}

pub fn load_all_assets_system(res: ResourcesHolderMut) {
    // todo: use fruits_file_storage::assets_path() instead of cwd-relative "assets"
    let mut assets_dir_path = PathBuf::new();
    assets_dir_path.push("assets");

    load_all_assets(res, assets_dir_path);
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AssetType {
    Texture,
    Material,
    Mesh,
    Font,
    AudioClip,
    Prefab,
}

impl AssetType {
    pub const fn serialized_str(&self) -> &'static str {
        match self {
            AssetType::Texture => "texture",
            AssetType::Material => "material",
            AssetType::Mesh => "mesh",
            AssetType::Font => "font",
            AssetType::AudioClip => "audio_clip",
            AssetType::Prefab => "prefab",
        }
    }

    pub fn from_serialized_str(serialized_str: &str) -> Option<Self> {
        match serialized_str {
            "texture" => Some(AssetType::Texture),
            "material" => Some(AssetType::Material),
            "mesh" => Some(AssetType::Mesh),
            "font" => Some(AssetType::Font),
            "audio_clip" => Some(AssetType::AudioClip),
            "prefab" => Some(AssetType::Prefab),
            _ => None
        }
    }
}

pub trait AssetLoader<S: Copy> {
    type Asset: 'static + Send + Sync;
    type SelfWithAnotherLifetime<'r>: 'r + AssetLoader<S, Asset = Self::Asset>;

    fn create_loader<'r>(res: ResourcesHolderMut<'r>) -> Option<Self::SelfWithAnotherLifetime<'r>>;

    fn load_from_serialized(&mut self, ctx: SerializerCtx<S>, value: &SerializedValue, assets_dir_path: impl AsRef<Path>) -> Option<Self::Asset>;
    fn get_related_asset_storage(&mut self) -> &mut AssetStorageResource<Self::Asset>;

    fn get_or_load_from_key(&mut self, ctx: SerializerCtx<S>, key: &str, assets_dir_path: impl AsRef<Path>) -> Option<AssetHandle<Self::Asset>> {
        let storage = self.get_related_asset_storage();

        if let Some(stored_asset) = storage.get_registered(key) {
            if storage.get(stored_asset).is_some() {
                return Some(stored_asset.clone());
            }

            storage.unregister(key);
        }

        let mut path = assets_dir_path.as_ref().to_path_buf();
        path.push(key);

        let raw_asset = match std::fs::read_to_string(path) {
            Ok(data) => data,
            Err(_err) => return None,
        };

        let raw_asset = SerializedValue::from_json(&serde_json::from_str::<serde_json::Value>(&raw_asset).ok()?);
        let Some(texture) = self.load_from_serialized(ctx, &raw_asset, &assets_dir_path) else {
            return None;
        };

        let storage = self.get_related_asset_storage();
        let asset_handle = storage.insert(texture);

        storage.register(key.into(), asset_handle.clone());

        Some(asset_handle)
    }
}

pub fn load_all_assets(mut res: ResourcesHolderMut, assets_dir_path: impl AsRef<Path>) {
    load_asset_transitively_from_world(
        res.as_mut(),
        assets_dir_path.as_ref(),
        None, 
        |serializers_local, serializers_global| {
            let mut err_handler = |err| println!("[{}:{}] {err}", file!(), line!());
            let serializer_ctx_state = serializers_global.to_ctx_state();
            let mut serializer_ctx = serializer_ctx_state.wrap_with_local(&serializers_local).into_ctx(&mut err_handler);
            
            traverse_files_in_dir_deep(&assets_dir_path, &mut |file_path| {
                if file_path.extension() != Some(OsStr::new("asset")) {
                    return;
                }

                if file_path.to_str().is_none() {
                    println!("failed to load asset at path {file_path:?}, path is not utf-8 friendly");
                    return;
                };

                let asset_key = file_path.components().skip(assets_dir_path.as_ref().components().count()).map(|c| c.as_os_str().to_str().unwrap()).collect::<Vec<_>>().join("/");

                let asset_json = match std::fs::read_to_string(&file_path) {
                    Ok(data) => data,
                    Err(err) => {
                        println!("failed to load asset at path {file_path:?}, file read error: {err}");
                        return;
                    },
                };

                let asset_json = match serde_json::from_str::<serde_json::Value>(&asset_json) {
                    Ok(data) => data,
                    Err(err) => {
                        println!("failed to load asset at path {file_path:?}, invalid json: {err}");
                        return;
                    }
                };

                let serde_json::Value::Object(asset_json_obj) = &asset_json else {
                    println!("failed to load asset at path {file_path:?}, asset should be a json object");
                    return;
                };

                let Some(serde_json::Value::String(asset_type)) = asset_json_obj.get("asset_type") else {
                    println!("failed to load asset at path {file_path:?}, asset_type is missing");
                    return;
                };

                let Some(asset_type) = AssetType::from_serialized_str(&asset_type) else {
                    println!("failed to load asset at path {file_path:?}, unsupported asset_type: {asset_type}");
                    return;
                };

                let serialized_value = SerializedPrimitive::String(asset_key.into()).into();

                match asset_type {
                    AssetType::Texture => _ = serializer_ctx.deserialize_default::<AssetHandle<StandardTexture>>("", &serialized_value),
                    AssetType::Material => _ = serializer_ctx.deserialize_default::<AssetHandle<StandardMaterial>>("", &serialized_value),
                    AssetType::Mesh => _ = serializer_ctx.deserialize_default::<AssetHandle<StandardMesh>>("", &serialized_value),
                    AssetType::AudioClip => _ = serializer_ctx.deserialize_default::<AssetHandle<AudioClip>>("", &serialized_value),
                    // todo: font
                    AssetType::Font => todo!(),
                    AssetType::Prefab => _ = serializer_ctx.deserialize_default::<AssetHandle<Prefab>>("", &serialized_value),
                };
            });
        },
    ).unwrap();
}

fn traverse_files_in_dir_deep(dir_path: impl AsRef<Path>, f: &mut impl FnMut(PathBuf)) {
    let Ok(dir) = std::fs::read_dir(&dir_path) else {
        return;
    };

    for entry in dir {
        let Ok(entry) = entry else {
            continue;
        };

        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_file() {
            f(entry.path());
        } else if file_type.is_dir() {
            traverse_files_in_dir_deep(entry.path(), f);
        }
    }
}

//

const _MATERIAL_FILE_EXAMPLE: &str = r##"
{
    "asset_type": "material",
    "material_type": "lit",
    "color": "#ffa641ff",
    "color_texture": "path/to/texture.asset",
    "metallic": 0.95,
    "roughness": 0.1,
    "space": "world"
}
"##;

const _TEXTURE_FILE_EXAMPLE: &str = r#"
{
    "asset_type": "texture",
    "raw_texture": "path/to/raw_texture.png",
    "format": "srgb"
}
"#;

const _MESH_FILE_EXAMPLE: &str = r#"
{
    "asset_type": "mesh",
    "raw_mesh": "path/to/raw_mesh.obj",
    "recalculate_normals": false,
    "force_shade_flat": false
}
"#;

const _FONT_FILE_EXAMPLE: &str = r##"
{
    "asset_type": "font",
    "texture": "path/to/texture.asset",
    "characters_uv": { "a": [[0.5, 0.7], [0.55, 0.75]] },
    "missing_character_uv": [[0.5, 0.7], [0.55, 0.75]],
    "character_ratio": 0.756
}
"##;

const _AUDIO_CLIP_FILE_EXAMPLE: &str = r##"
{
    "asset_type": "audio",
    "raw_audio": "path/to/audio.asset"
}
"##;
