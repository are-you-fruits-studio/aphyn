# Prioritized

- implement new asset: scene
- rework rendering to be able to implement both scene and preview windows
- implement playable game window ("Preview") (with simulated world being live and using updates)
- make OS window icon configurable
- implement global cpu-stored batched mesh (same batching as now, but using lightweight pattern)
- make components rendering prettier and make components look separated
- implement the ability to use custom shaders
- implement the ability to create assets from editor
- implement dropout for components, entities and folders

# Default

- make sure prefab root entity is the first one serialized

- implement windows docking (like in Microsoft VS)
- copy-paste (with right mouse button and drop-down) (maybe even ctrl c ctrl v)
- transform-adjust interactable gizmo on selected object
- fancy contrasting borders on windows
- performance profiler window
- console window
- implement systems scheduler where threadpool asks iterator for systems
- show opened asset with different kind of selection
- live reload assets
- live recompile scripts
- use pure serializer if no transserializer available
- maybe add scroll with automatic visibility to dynamic dropdown
- restrict windows layout to not go outside bounds
- implement more basic features for input fields (cursor, selection, shortcuts, etc.)
- describe project structure and architecture better for claude
- order windows on the z axis based on what was used more recently
- save last window title change, so that this api isn't cleared by OS that don't support title reading: W11, iOS and other platforms
- other "todo"s throughout the project

# Low-prio

- allow batch-adding components to entities and spawning entities directly into the right archetype (optimization)