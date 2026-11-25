use aphyn_asset_storage::AssetHandle;
use aphyn_ecs::Component;
use aphyn_ffi::{FfiString, FfiVec};
use aphyn_math::{Vec2, Vec4};
use aphyn_reflection::*;
use aphyn_serialization::*;
use aphyn_transform::UiVal;

use crate::assets::Font;

#[repr(C)]
#[derive(Component, Serializable, Reflectable)]
pub struct TextComponent {
    pub font: AssetHandle<Font>,
    pub text: FfiString,
    pub font_size: UiVal,
    pub horizontal_align: HorizontalAlign,
    pub vertical_align: VerticalAlign,
    pub is_y_inverted: bool,
    pub horizontal_spacing: UiVal,
    pub color: Vec4<f32>,
}
impl Default for TextComponent {
    fn default() -> Self {
        Self {
            font: AssetHandle::EMPTY,
            text: FfiString::new(),
            font_size: UiVal::px(20.0),
            horizontal_align: HorizontalAlign::Left,
            vertical_align: VerticalAlign::Middle,
            is_y_inverted: true,
            horizontal_spacing: UiVal::px(0.0),
            color: Vec4::splat(1.0),
        }
    }
}

#[repr(C)]
#[derive(Component, Serializable, Reflectable)]
pub struct ImageComponent {
    pub is_y_inverted: bool,
    pub color: Vec4<f32>,
    pub fill_amt: f32,
    pub fill_settings: ImageFillSettings,
}
impl Default for ImageComponent {
    fn default() -> Self {
        Self {
            is_y_inverted: true,
            color: Vec4::splat(1.0),
            fill_amt: 1.0,
            fill_settings: ImageFillSettings::RadialCenter,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default, Serializable, Reflectable)]
pub enum ImageFillSettings {
    // todo: add more variants and details.
    #[default]
    RadialCenter,
}

#[repr(C)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serializable, Reflectable)]
pub enum HorizontalAlign {
    #[default]
    Left,
    Middle,
    Right,
}

#[repr(C)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serializable, Reflectable)]
pub enum VerticalAlign {
    Top,
    #[default]
    Middle,
    Bottom,
}

#[repr(C)]
#[derive(Component, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serializable, Reflectable)]
pub struct ChildrenRectMaskComponent;

#[repr(C)]
#[derive(Component, Serializable, Reflectable)]
pub struct UiLineComponent {
    // in the entity's rect: (0, 0) is its min corner, (1, 1) its max corner
    pub points: FfiVec<Vec2<f32>>,
    pub thickness_px: f32,
    // draws points as separate (start, end) pairs instead of one connected strip
    pub is_segments: bool,
    pub color: Vec4<f32>,
}
impl Default for UiLineComponent {
    fn default() -> Self {
        Self {
            points: FfiVec::new(),
            thickness_px: 1.0,
            is_segments: false,
            color: Vec4::splat(1.0),
        }
    }
}
