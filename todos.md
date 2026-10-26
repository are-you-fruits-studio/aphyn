# Prioritized

- Make material updateable at runtime and optimize its gpu-buffer writes to dirty-only
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