# Prioritized

- Make material updateable at runtime and optimize its gpu-buffer writes to dirty-only
    - (already implemented for textures) add id to texture and other assets (another level of indirection to know when a wgpu texture has changed even when AssetHandle is the same).
    - separate ui render space into separate engine shaders. Remove it from asset metadata. Move world_to_clip and window_to_clip as 2 distinct matrices to global uniform. Make shaders use different matrices based on whether they are ui or non-ui shaders. The new shaders are UiOpaqueUnlit, UiTransparentUnlit - no need for the lit ui.
    - implement change detection on Material asset metadata. Shader field doesn't need change detection. Asset Handles change-detection should be based on u64 ids of Textures. Other fields's change detection should be implemented using simple is_dirty flag (meta_mut() method automatically triggers the is_dirty flag).
    - rewrite Material uniform buffer only on changes to uniforms in asset metadata and recreate Material bind group only on changes to the textures in asset metadata. Probably, it's better to move this to Material (new method update_gpu_data_if_dirty - if is_dirty it writes the uniform buffer, recreates the bind group and resets the is_dirty flag).
    - remove StandardMaterialAssets from StandardMaterial::new()

- make sure prefab root entity is the first one serialized

# Default

- implement windows docking (like in Microsoft VS)
- copy-paste (with right mouse button and drop-down) (maybe even ctrl c ctrl v)
- performance profiler window
- live reload assets
- live recompile scripts

# Queue

- implement systems scheduler where threadpool asks iterator for systems
    - don't create threadpool per systems executor. Make it external tool that is used as a parameter for every execute_iteration call
    - measure how threadpool is used
    - Make world execution configurable - single-threaded or multi-threaded

- improve playmode performance:
    - optimize update_hierarchy_window_system

- transform-adjust interactable gizmo on selected object
- console window
- drag-drop files to move/assign them
- show opened asset with different kind of selection
- describe project structure and architecture better for claude
- implement more basic features for input fields (cursor, selection, shortcuts, etc.)
- make sure scenes can be spawned
- implement animation
- implement shadows
- implement Skybox
- implement SSR (Screen-Space Reflections)
- make rendering more custimizeable
- implement physics
- implement initializing the window in the app builder before the window has been created
- implement initializing app_id for the window

# Low-prio

- allow  spawning entities directly into the right archetype (optimization)
- implement global cpu-stored batched mesh (same batching as now, but using lightweight pattern) (Let's make StandardMesh store the FfiVec<StandardVertex> vertices and FfiVec<u16> indices after creation. Let's also put it vertices and indices into StandardMeshData and use it in StandardMesh and BatchedMeshComponent instead of directly storing vertices and indices.)
- use pure serializer if no transserializer available
- maybe add scroll with automatic visibility to dynamic dropdown
- other "todo"s throughout the project
- support saving assets (and project files changes: create/rename/delete) in play mode