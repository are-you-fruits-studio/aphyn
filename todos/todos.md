# Prioritized

- refactor serialization (optimize)
    - separate ContextData (different per save/load approach) from SerializersRegistry (immutable)
    - register serializers at app loading, build it and use as immutable in systems and render-features
    - implement reflection
    - implement inspector
- separate serialization into reflection and inspector:
    - There are 3 sides to it, and all work with the runtime type, not between each other:
        - reflection (update/read values)
        - serialization (converting between serialized values and runtime typed values)
        - inspecting (edit and visualize the type values with user-friendly visuals)
    - All 3 are registerable at the app init and then are built and set in stone
    - Reflection is the main one - it's used for animation and potentially prefabs. Other sides can figure out fallback behaviour based on reflection. Without it the type cannot participate in inspector and animation. It's main goal is to edit/read the type data without knowing the type at compile time.
    - Serialization is used to save and restore the type. If reflection is registered, serialization is optional. Serializarion is done like today with additional optimizations.
    - Inspecting is used to edit the type by user. If reflection is registered, inspecting is optional. Inspecting is done using custom imgui.
- implement custom inspectors. Inspecting ideally can be used in multiple layers of simplicity/detail:
    - describe field names and their getters/setters
    - use simplified ui commands (like imgui) and change inspected data directly
    - describe a ui and change inspected data directly
    - What inspecting should ideally support:
        - labels
        - (draggable) input fields
        - checkbox
        - buttons
        - horizontal/vertical layout
        - scrollview
        - dropdown
        - slider
        - color picker
        - collections?
        - curve?
    - Enums have different structure based on variants. Collections have different count based on length. 
    - Imgui is interesting here because it can easily react to changes in variant dropdown and update the contents accordingly.
    - Html-css-like ui will have to notice user input, delete old fields and spawn new ones. It's more conventional, but less convenient.
    - The whole editor is not obligated to use the inspecting thing - it can draw itself with ecs. Inspecting tool is only used in the inspector.
    - So imgui shouldn't have universal api, it should expose a list of currently supported tools (layout, slider, checkbox, etc.) to quickly draw the inspector

- Create ffi type id (TypeRegistry-depending) and use it instead of the typename where possible? Also integrate it into ecs

- make sure prefab root entity is the first one serialized

- implement Skybox (probably need separation of vertex/fragment shaders in assets)
- implement animation
- implement engine assets (like in Unity they have Assets folder and Packages folder in the Project window).
- show opened asset with different kind of selection
- drag-drop files to move/assign them
- visualize pretty/conveniently the other types of serialized values (bool as checkbox, color as a color selector, etc.)
- fbx import
- fonts and their import

- resources serialization (or resources as assets)
    - What holds serialized resources? Scene? ResourcesScene? Maybe I should rename current Scene into MultiPrefab and create a separate scene that contains entities+components AND resources?
    - What do I need scenes for? Scenes are useful to playtest different setups. Or are they? What more is there in terms of their use-cases? Are scenes usable to be spawned manually at runtime? Or are they for app-init only?
    - If scenes are only for initing the app/world/"scene?", then they should contain both entities+components AND resources. Should they also contain systems & render-features in this case?
    - Make serialization use registry builder that gathers serializers and on build builds an immutable registry with a dependency-tree inside. Don't use local serializer_registry, use context. The context is a typed registry of virtual functions that some registered serializers can use. The local serializers are only used for EntityId, AssetHandle and direct asset types - all these can be handled with local context instead.

# Queue

- SSR
- SSGI
- SSAO
- make sure skybox affects SSGI and SSR
- optional Deferred rendering
- instantiate prefabs in editor (probably need to support prefabs as part of other prefabs and overrides)
- convenient LayerMask inspector
- console window
- frustum culling

# Not prioritized yet

- Materials that have configurations that change render pipeline configuration (e.g. culling)
- refactor/optimize/make-prettier lib compilation waiting
- undo in editor
- compact vector visualization in inspector
- when interacting with scene window, stop editing input fields

# Low-prio

- describe project structure and architecture better for claude
- implement SSR (Screen-Space Reflections)

- improve editor playmode performance:
    - optimize update_hierarchy_window_system

- implement FXAA (Fast Approximate Anti-Aliasing)
- view the game with the highlited mesh triangles' edges
- allow spawning entities directly into the right archetype (optimization)
- use pure serializer if no transserializer available
- maybe add scroll with automatic visibility to dynamic dropdown
- other "todo"s throughout the project
- support saving assets (and project files changes: create/rename/delete) in play mode
- performance profiler window
- dynamic shadowmap allocations inside shadowmap atlas
- point light shadows (cube faces in the shadow atlas)