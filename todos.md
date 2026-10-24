# Prioritized

- implement systems scheduler where threadpool asks iterator for systems
- improve playmode performance:
    - optimize update_hierarchy_window_system

- implement the ability to use custom shaders

import full shaders from code,
import depending/full shaders from assets,

- make sure prefab root entity is the first one serialized
- implement windows docking (like in Microsoft VS)
- copy-paste (with right mouse button and drop-down) (maybe even ctrl c ctrl v)
- performance profiler window
- live reload assets
- live recompile scripts

# Default


- transform-adjust interactable gizmo on selected object
- console window

- drag-drop files to move/assign them
- show opened asset with different kind of selection
- describe project structure and architecture better for claude
- implement more basic features for input fields (cursor, selection, shortcuts, etc.)

# Low-prio

- allow batch-adding components to entities and spawning entities directly into the right archetype (optimization)
- implement global cpu-stored batched mesh (same batching as now, but using lightweight pattern) (Let's make StandardMesh store the FfiVec<StandardVertex> vertices and FfiVec<u16> indices after creation. Let's also put it vertices and indices into StandardMeshData and use it in StandardMesh and BatchedMeshComponent instead of directly storing vertices and indices.)
- use pure serializer if no transserializer available
- maybe add scroll with automatic visibility to dynamic dropdown
- other "todo"s throughout the project
- support saving assets (and project files changes: create/rename/delete) in play mode