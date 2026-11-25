use aphyn::*;

use crate::{
    SYSTEM_GROUP,
    features::{
        inspector_window::systems::{
            save_inspected_asset_from_simulated_world_to_file_system, save_simulated_entities_to_asset_system,
        },
        scene_window::control_scene_camera_system,
        ui_interaction::{ButtonComponent, check_ui_interaction_system},
        world_preload::tick_simulated_world_system,
    },
};

mod handles;
mod interaction;
mod mesh;
mod render;

pub use interaction::transform_gizmo_interaction_system;
pub use mesh::build_transform_gizmo_mesh;
pub use render::{TransformGizmoMeshResource, TransformGizmoRenderFeature};

pub const TRANSFORM_GIZMO_RENDER_FEATURE: &str = "aphyn_editor/transform_gizmo";

const BUTTON_COLOR: Vec4<f32> = Vec4::new(0.3, 0.3, 0.3, 1.0);
const BUTTON_COLOR_ACTIVE: Vec4<f32> = Vec4::from_array(const { parse_color_rgba_f32("#7d90e6ff").unwrap() });

pub fn register_feature(ctx: FeatureInitCtx) {
    let FeatureInitCtx {
        mut data, mut behavior, ..
    } = ctx;
    data.resources_mut().insert(TransformGizmoResource::default());

    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(click_transform_gizmo_toolbar_system)
        .insert_child_system(update_transform_gizmo_toolbar_system)
        .insert_child_system(transform_gizmo_interaction_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(click_transform_gizmo_toolbar_system);
    update
        .order_system(click_transform_gizmo_toolbar_system)
        .before_system(update_transform_gizmo_toolbar_system);
    update
        .order_system(click_transform_gizmo_toolbar_system)
        .before_system(transform_gizmo_interaction_system);
    update
        .order_system(control_scene_camera_system)
        .before_system(transform_gizmo_interaction_system);
    update
        .order_system(transform_gizmo_interaction_system)
        .before_system(tick_simulated_world_system);
    update
        .order_system(transform_gizmo_interaction_system)
        .before_system(save_simulated_entities_to_asset_system);
    update
        .order_system(transform_gizmo_interaction_system)
        .before_system(save_inspected_asset_from_simulated_world_to_file_system);
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum GizmoTool {
    None,
    #[default]
    Move,
    Rotate,
    Scale,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum GizmoSpace {
    #[default]
    Global,
    Local,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum GizmoHandle {
    Axis(usize),
    Plane(usize),
    Center,
    ViewRing,
}

#[derive(Resource, Default)]
pub struct TransformGizmoResource {
    pub tool: GizmoTool,
    pub space: GizmoSpace,
    hovered: Option<GizmoHandle>,
    drag: Option<interaction::GizmoDrag>,
}

impl TransformGizmoResource {
    fn active_handle(&self) -> Option<GizmoHandle> {
        self.drag.as_ref().map(|drag| drag.handle).or(self.hovered)
    }
}

#[derive(Component, Default, Debug)]
pub struct GizmoToolButtonComponent {
    pub tool: GizmoTool,
}

#[derive(Component, Default, Debug)]
pub struct GizmoSpaceButtonComponent;

fn click_transform_gizmo_toolbar_system(
    tool_button_q: WorldQuery<(&ButtonComponent, &GizmoToolButtonComponent)>,
    space_button_q: WorldQuery<&ButtonComponent, WithFilter<GizmoSpaceButtonComponent>>,
    mut gizmo_res: ResMut<TransformGizmoResource>,
) {
    if let Some((_, tool_button_c)) = tool_button_q.iter().find(|(button_c, _)| button_c.was_clicked_this_frame) {
        gizmo_res.tool = tool_button_c.tool;
        gizmo_res.drag = None;
    }

    if space_button_q.iter().any(|button_c| button_c.was_clicked_this_frame) {
        gizmo_res.space = match gizmo_res.space {
            GizmoSpace::Global => GizmoSpace::Local,
            GizmoSpace::Local => GizmoSpace::Global,
        };
        gizmo_res.drag = None;
    }
}

fn update_transform_gizmo_toolbar_system(mut ent: EntitiesHolderMut, gizmo_res: Res<TransformGizmoResource>) {
    let tool_buttons = ent
        .query::<(EntityId, &GizmoToolButtonComponent)>()
        .iter()
        .map(|(ent_button, button_c)| (ent_button, button_c.tool))
        .collect::<Vec<_>>();

    for (ent_button, tool) in tool_buttons {
        if let Some(image_c) = ent.get_component_mut::<ImageComponent>(ent_button) {
            image_c.color = if tool == gizmo_res.tool {
                BUTTON_COLOR_ACTIVE
            } else {
                BUTTON_COLOR
            };
        }
    }

    let space_buttons = ent
        .query_filtered::<EntityId, WithFilter<GizmoSpaceButtonComponent>>()
        .iter()
        .collect::<Vec<_>>();

    let space_text = match gizmo_res.space {
        GizmoSpace::Global => "Global",
        GizmoSpace::Local => "Local",
    };

    for ent_button in space_buttons {
        let children = ent
            .get_component::<ParentComponent>(ent_button)
            .map(|p| p.children().to_vec())
            .unwrap_or_default();

        for ent_text in children {
            continue_if_not!(Some(text_c) = ent.get_component_mut::<TextComponent>(ent_text));

            if text_c.text.as_str() != space_text {
                text_c.text.clear();
                text_c.text.push_str(space_text);
            }
        }
    }
}
