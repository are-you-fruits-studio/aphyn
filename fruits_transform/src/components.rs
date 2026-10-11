use fruits_ecs::{Component, EntityId};
use fruits_ffi::{FfiOption, FfiVec};
use fruits_math::{Mat, Mat3, Quat, Vec2, Vec3};
use fruits_serialization::*;
use serde::{Deserialize, Serialize};

#[repr(C)]
#[derive(Component, Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GlobalTransform {
    pub position: Vec3<f32>,
    pub scale_rotation: Mat3<f32>,
}
impl GlobalTransform {
    pub const IDENTITY: GlobalTransform = GlobalTransform {
        position: Vec3::splat(0.0),
        scale_rotation: Mat::IDENTITY,
    };
}
impl Default for GlobalTransform {
    fn default() -> Self {
        GlobalTransform::IDENTITY
    }
}

#[repr(C)]
#[derive(Component, Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalTransform {
    pub position: Vec3<f32>,
    pub rotation: Quat<f32>,
    pub scale: Vec3<f32>,
}
impl LocalTransform {
    pub const IDENTITY: Self = Self {
        position: Vec3::splat(0.0),
        rotation: Quat::IDENTITY,
        scale: Vec3::splat(1.0),
    };
}
impl Default for LocalTransform {
    fn default() -> Self {
        LocalTransform::IDENTITY
    }
}

#[repr(C)]
#[derive(Component, Clone, Debug, PartialEq, Default, Serializable)]
pub struct ParentComponent {
    pub children: FfiVec<EntityId>,
}

#[repr(C)]
#[derive(Default, Component, Copy, Clone, Debug, PartialEq, Serializable)]
pub struct ChildComponent {
    pub parent: EntityId,
}

#[repr(C)]
#[derive(Component, Copy, Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct GlobalRectComponent {
    pub center: Vec2<f32>,
    pub scale: Vec2<f32>,
    pub z: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiDirection {
    Vertical,
    Horizontal,
}

impl UiDirection {
    pub const fn to_axis_idx(self) -> usize {
        match self {
            UiDirection::Horizontal => 0,
            UiDirection::Vertical => 1,
        }
    }
}

#[repr(C)]
#[derive(Component, Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LocalRectComponent {
    pub parent_padding_min: Vec2<UiVal>,
    pub parent_padding_max: Vec2<UiVal>,
    pub offset: Vec2<UiVal>,
    pub anchor: Vec2<f32>,
    pub pivot: Vec2<f32>,
    pub scale: Vec2<FfiOption<UiVal>>,
    pub z: f32,
}
impl Default for LocalRectComponent {
    fn default() -> Self {
        Self {
            parent_padding_min: Vec2::splat(UiVal::px(0.0)),
            parent_padding_max: Vec2::splat(UiVal::px(0.0)),
            offset: Vec2::splat(UiVal::px(0.0)),
            anchor: Vec2::splat(0.5),
            pivot: Vec2::splat(0.5),
            scale: Vec2::new(Some(UiVal::pw(1.0)), Some(UiVal::ph(1.0))).map(Into::into),
            z: -1.0,
        }
    }
}

#[repr(C)]
#[derive(Component, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct GlobalDisableableComponent {
    pub is_disabled: bool,
}
impl Default for GlobalDisableableComponent {
    fn default() -> Self {
        Self { is_disabled: false }
    }
}
#[repr(C)]
#[derive(Component, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub struct LocalDisableableComponent {
    pub is_disabled: bool,
}
impl Default for LocalDisableableComponent {
    fn default() -> Self {
        Self { is_disabled: false }
    }
}

#[repr(C)]
#[derive(Component, Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RectChildAlignComponent {
    pub anchor: Vec2<f32>,
    pub direction: UiDirection,
    pub min_gap: UiVal,
    pub spacing: UiSpacing,
}

impl Default for RectChildAlignComponent {
    fn default() -> Self {
        Self {
            anchor: Vec2::splat(0.5),
            direction: UiDirection::Vertical,
            min_gap: UiVal::px(0.0),
            spacing: UiSpacing::SpaceBetween,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UiSpacing {
    Chunk,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiVal {
    pub unit: UiUnit,
    pub val: f32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum UiUnit {
    Px,
    Pw,
    Ph,
    Pd,
    Pmin,
    Pmax,
    Vw,
    Vh,
    Vd,
    Vmin,
    Vmax,
}

impl UiVal {
    pub const fn px(v: f32) -> Self {
        Self {
            unit: UiUnit::Px,
            val: v,
        }
    }
    pub const fn pw(v: f32) -> Self {
        Self {
            unit: UiUnit::Pw,
            val: v,
        }
    }
    pub const fn ph(v: f32) -> Self {
        Self {
            unit: UiUnit::Ph,
            val: v,
        }
    }
    pub const fn pd(v: f32) -> Self {
        Self {
            unit: UiUnit::Pd,
            val: v,
        }
    }
    pub const fn pmin(v: f32) -> Self {
        Self {
            unit: UiUnit::Pmin,
            val: v,
        }
    }
    pub const fn pmax(v: f32) -> Self {
        Self {
            unit: UiUnit::Pmax,
            val: v,
        }
    }
    pub const fn vw(v: f32) -> Self {
        Self {
            unit: UiUnit::Vw,
            val: v,
        }
    }
    pub const fn vh(v: f32) -> Self {
        Self {
            unit: UiUnit::Vh,
            val: v,
        }
    }
    pub const fn vd(v: f32) -> Self {
        Self {
            unit: UiUnit::Vd,
            val: v,
        }
    }
    pub const fn vmin(v: f32) -> Self {
        Self {
            unit: UiUnit::Vmin,
            val: v,
        }
    }
    pub const fn vmax(v: f32) -> Self {
        Self {
            unit: UiUnit::Vmax,
            val: v,
        }
    }
}

impl UiVal {
    pub fn into_px(self, parent_size: Vec2<f32>, view_size: Vec2<f32>) -> Vec2<f32> {
        let v = self.val;

        match self.unit {
            UiUnit::Px => Vec2::splat(v),
            UiUnit::Pw => Vec2::splat(parent_size.x * v),
            UiUnit::Ph => Vec2::splat(parent_size.y * v),
            UiUnit::Pd => parent_size * v,
            UiUnit::Pmin => Vec2::splat(f32::min(parent_size.x, parent_size.y) * v),
            UiUnit::Pmax => Vec2::splat(f32::max(parent_size.x, parent_size.y) * v),
            UiUnit::Vw => Vec2::splat(view_size.x * v),
            UiUnit::Vh => Vec2::splat(view_size.y * v),
            UiUnit::Vd => view_size * v,
            UiUnit::Vmin => Vec2::splat(f32::min(view_size.x, view_size.y) * v),
            UiUnit::Vmax => Vec2::splat(f32::max(view_size.x, view_size.y) * v),
        }
    }

    pub fn into_px_without_parent(self, view_size: Vec2<f32>) -> Option<Vec2<f32>> {
        let v = self.val;

        Some(match self.unit {
            UiUnit::Px => Vec2::splat(v),
            UiUnit::Vw => Vec2::splat(view_size.x * v),
            UiUnit::Vh => Vec2::splat(view_size.y * v),
            UiUnit::Vd => view_size * v,
            UiUnit::Vmin => Vec2::splat(f32::min(view_size.x, view_size.y) * v),
            UiUnit::Vmax => Vec2::splat(f32::max(view_size.x, view_size.y) * v),
            _ => return None,
        })
    }
}

impl Default for UiVal {
    fn default() -> Self {
        Self::px(0.0)
    }
}
