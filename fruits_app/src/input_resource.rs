use fruits_ecs::Resource;
use fruits_ffi::FfiIndexMap;
use fruits_ffi::FfiIndexSet;
use gilrs::Axis;

pub use gilrs::Axis as GamepadAxis;
pub use gilrs::Button as GamepadButton;
pub use winit::event::MouseButton;
pub use winit::keyboard::KeyCode;

#[repr(C)]
#[derive(Resource, Clone, Default)]
pub struct InputResource {
    pub keyboard: KeyboardInputStorage,
    pub mouse: MouseInputStorage,
    pub gamepads: FfiIndexMap<usize, GamepadInputStorage>,
}

impl InputResource {
    pub fn clear_frame(&mut self) {
        self.keyboard.clear_frame();
        self.mouse.clear_frame();
        self.gamepads.values_mut().for_each(|g| g.clear_frame());
    }
}

//

#[repr(C)]
#[derive(Clone, Default)]
pub struct KeyboardInputStorage {
    pressed: FfiIndexSet<KeyCode>,
    frame_pressed: FfiIndexSet<KeyCode>,
    frame_released: FfiIndexSet<KeyCode>,
}

impl KeyboardInputStorage {
    pub fn is_pressed(&self, k: KeyCode) -> bool {
        self.pressed.contains(&k)
    }

    pub fn is_just_pressed(&self, k: KeyCode) -> bool {
        self.frame_pressed.contains(&k)
    }

    pub fn is_just_released(&self, k: KeyCode) -> bool {
        self.frame_released.contains(&k)
    }

    pub fn press(&mut self, k: KeyCode) {
        self.pressed.insert(k);
        self.frame_pressed.insert(k);
    }

    pub fn release(&mut self, k: KeyCode) {
        self.pressed.remove_swap(&k);
        self.frame_released.insert(k);
    }

    pub fn clear(&mut self) {
        self.pressed.clear();
        self.frame_pressed.clear();
        self.frame_released.clear();
    }

    pub fn clear_frame(&mut self) {
        self.frame_pressed.clear();
        self.frame_released.clear();
    }
}

//

#[repr(C)]
#[derive(Clone, Default)]
pub struct MouseInputStorage {
    pressed: FfiIndexSet<MouseButton>,
    frame_pressed: FfiIndexSet<MouseButton>,
    frame_released: FfiIndexSet<MouseButton>,
    pub position: [f64; 2],
    pub scroll_delta_lines: [f32; 2],
    pub scroll_delta_pixels: [f64; 2],
}

impl MouseInputStorage {
    pub fn is_pressed(&self, k: MouseButton) -> bool {
        self.pressed.contains(&k)
    }

    pub fn is_just_pressed(&self, k: MouseButton) -> bool {
        self.frame_pressed.contains(&k)
    }

    pub fn is_just_released(&self, k: MouseButton) -> bool {
        self.frame_released.contains(&k)
    }

    pub fn press(&mut self, k: MouseButton) {
        self.pressed.insert(k);
        self.frame_pressed.insert(k);
    }

    pub fn release(&mut self, k: MouseButton) {
        self.pressed.remove_swap(&k);
        self.frame_released.insert(k);
    }

    pub fn clear(&mut self) {
        self.pressed.clear();
        self.clear_frame();
    }

    pub fn clear_frame(&mut self) {
        self.frame_pressed.clear();
        self.frame_released.clear();
        self.scroll_delta_lines = [0.0; 2];
        self.scroll_delta_pixels = [0.0; 2];
    }
}

//

#[repr(C)]
#[derive(Clone)]
pub struct GamepadInputStorage {
    pressed: FfiIndexSet<GamepadButton>,
    frame_pressed: FfiIndexSet<GamepadButton>,
    frame_released: FfiIndexSet<GamepadButton>,
    axes: FfiIndexMap<Axis, f32>,
}

impl GamepadInputStorage {
    pub fn new() -> Self {
        Self {
            pressed: Default::default(),
            frame_pressed: Default::default(),
            frame_released: Default::default(),
            axes: Default::default(),
        }
    }

    pub fn is_pressed(&self, k: GamepadButton) -> bool {
        self.pressed.contains(&k)
    }

    pub fn is_just_pressed(&self, k: GamepadButton) -> bool {
        self.frame_pressed.contains(&k)
    }

    pub fn is_just_released(&self, k: GamepadButton) -> bool {
        self.frame_released.contains(&k)
    }

    pub fn read_axis(&self, a: Axis) -> f32 {
        self.axes.get(&a).copied().unwrap_or_default()
    }

    pub fn write_axis(&mut self, a: Axis, v: f32) {
        self.axes.insert(a, v);
    }

    pub fn press(&mut self, k: GamepadButton) {
        self.pressed.insert(k);
        self.frame_pressed.insert(k);
    }

    pub fn release(&mut self, k: GamepadButton) {
        self.pressed.remove_swap(&k);
        self.frame_released.insert(k);
    }

    pub fn clear(&mut self) {
        self.pressed.clear();
        self.axes.clear();
        self.frame_pressed.clear();
        self.frame_released.clear();
    }

    pub fn clear_frame(&mut self) {
        self.frame_pressed.clear();
        self.frame_released.clear();
    }
}
