# Prioritized

- copy-paste (with right mouse button and drop-down) (maybe even ctrl c ctrl v)
- ambient lighting
- view the game with the highlited mesh triangles' edges
- make sure prefab root entity is the first one serialized
- implement windows docking (like in Microsoft VS)
- rename entities (or make it so that all the entities in the editor have a DebugName component)

- instantiate prefabs in editor
- implement engine assets (like in Unity they have Assets folder and Packages folder in the Project window).
- resources serialization (or resources as assets)
- implement shadows
- implement Skybox
- implement animation

# Default

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

- allow spawning entities directly into the right archetype (optimization)
- use pure serializer if no transserializer available
- maybe add scroll with automatic visibility to dynamic dropdown
- other "todo"s throughout the project
- support saving assets (and project files changes: create/rename/delete) in play mode
- performance profiler window