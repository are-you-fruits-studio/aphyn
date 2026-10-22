# Prioritized

- implement systems scheduler where threadpool asks iterator for systems
- improve playmode performance:
    - optimize update_hierarchy_window_system

- fix hierarchy tree

- make OS window icon configurable
- save last window title change, so that this api isn't cleared by OS that don't support title reading: W11, iOS and other platforms

- implement the ability to use custom shaders

# Default

- make sure prefab root entity is the first one serialized
- support saving assets (and project files changes: create/rename/delete) in play mode

- drag-drop files to move/assign them
- implement global cpu-stored batched mesh (same batching as now, but using lightweight pattern) (Let's make StandardMesh store the FfiVec<StandardVertex> vertices and FfiVec<u16> indices after creation. Let's also put it vertices and indices into StandardMeshData and use it in StandardMesh and BatchedMeshComponent instead of directly storing vertices and indices.)
- implement windows docking (like in Microsoft VS)
- copy-paste (with right mouse button and drop-down) (maybe even ctrl c ctrl v)
- transform-adjust interactable gizmo on selected object
- performance profiler window
- console window
- show opened asset with different kind of selection
- live reload assets
- live recompile scripts
- use pure serializer if no transserializer available
- maybe add scroll with automatic visibility to dynamic dropdown
- implement more basic features for input fields (cursor, selection, shortcuts, etc.)
- describe project structure and architecture better for claude
- other "todo"s throughout the project

# Low-prio

- allow batch-adding components to entities and spawning entities directly into the right archetype (optimization)