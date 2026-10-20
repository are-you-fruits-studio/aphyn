# Prioritized

- implement global cpu-stored batched mesh (same batching as now, but using lightweight pattern)
- make components rendering prettier and make components look separated
- implement the ability to use custom shaders
- implement the ability to create assets from editor
- make simulated world use window parameters of the editor "Scene"/"Game" window, not the whole OS fruits_editor window
- change "winit window" window name to "fruits_engine app"
- when deserializing assets, try to deserialize them into existing asset handles, not recreating them (so that asset handles already depending on them stay valid)
- implement dropout for components, entities and folders

# Other

- implement windows docking (like in Microsoft VS)
- implement new asset: scene
- preview window
- rework rendering to be able to implement both scene and preview windows
- implement playable game window ("Preview") (with simulated world being live and using updates)
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
- use dynamic dropdown for enums inspecting
- maybe add scroll with automatic visibility to dynamic dropdown
- restrict windows layout to not go outside bounds
- make OS window name configurable
- implement more basic features for input fields (cursor, selection, shortcuts, etc.)
- describe project structure and architecture better for claude
- order windows on the z axis based on what was used more recently
- other "todo"s throughout the project