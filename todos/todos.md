# Prioritized

- make sure prefab root entity is the first one serialized

- implement shadows
- implement Skybox (probably need separation of vertex/fragment shaders in assets)
- implement animation
- instantiate prefabs in editor (probably need to support prefabs as part of other prefabs and overrides)
- implement engine assets (like in Unity they have Assets folder and Packages folder in the Project window).

- resources serialization (or resources as assets)
    - What holds serialized resources? Scene? ResourcesScene? Maybe I should rename current Scene into MultiPrefab and create a separate scene that contains entities+components AND resources?
    - What do I need scenes for? Scenes are useful to playtest different setups. Or are they? What more is there in terms of their use-cases? Are scenes usable to be spawned manually at runtime? Or are they for app-init only?
    - If scenes are only for initing the app/world/"scene?", then they should contain both entities+components AND resources. Should they also contain systems & render-features in this case?

# Default

- implement FXAA (Fast Approximate Anti-Aliasing)
- implement SSR (Screen-Space Reflections)

# Queue

- console window
- drag-drop files to move/assign them
- show opened asset with different kind of selection
- describe project structure and architecture better for claude

- fbx import
- fonts and their import

# Low-prio

- improve editor playmode performance:
    - optimize update_hierarchy_window_system

- view the game with the highlited mesh triangles' edges
- allow spawning entities directly into the right archetype (optimization)
- use pure serializer if no transserializer available
- maybe add scroll with automatic visibility to dynamic dropdown
- other "todo"s throughout the project
- support saving assets (and project files changes: create/rename/delete) in play mode
- performance profiler window