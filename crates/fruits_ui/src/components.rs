use fruits_asset_storage::AssetHandle;
use fruits_ecs::Component;
use fruits_ffi::FfiString;
use fruits_math::Vec4;
use fruits_serialization::*;
use fruits_transform::UiVal;

use crate::assets::Font;

#[repr(C)]
#[derive(Component, Serializable)]
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
#[derive(Component, Serializable)]
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
#[derive(Copy, Clone, Default, Serializable)]
pub enum ImageFillSettings {
    // todo: add more variants and details.
    #[default]
    RadialCenter,
}

#[repr(C)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serializable)]
pub enum HorizontalAlign {
    #[default]
    Left,
    Middle,
    Right,
}

#[repr(C)]
#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, Default, Serializable)]
pub enum VerticalAlign {
    Top,
    #[default]
    Middle,
    Bottom,
}

#[repr(C)]
#[derive(Component, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default, Serializable)]
pub struct ChildrenRectMaskComponent;
