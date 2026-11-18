use std::path::PathBuf;

use aphyn::*;

use crate::{
    SYSTEM_GROUP, StandardAssetsResource,
    features::{
        input_field::{
            InputFieldComponent, InputFieldTextChangedEvent, apply_input_field_context_menu_system,
            update_selected_input_field_text_system,
        },
        inspector_window::data::OpenProjectResource,
        scene_window::{SceneViewImageComponent, render_scene_window_system},
        ui_interaction::{ButtonComponent, check_ui_interaction_system},
    },
    prefabs::spawn_scene_light_panel,
};

const SETTINGS_DIR: &str = "editor";
const SETTINGS_FILE_NAME: &str = "scene_view.json";

const DEFAULT_LIGHT_COLOR: Vec3<f32> = Vec3::splat(3.0);

const BUTTON_COLOR: Vec4<f32> = Vec4::new(0.3, 0.3, 0.3, 1.0);
const BUTTON_COLOR_ACTIVE: Vec4<f32> = Vec4::from_array(const { parse_color_rgba_f32("#7d90e6ff").unwrap() });

// todo: bundle registration params
pub fn register_feature(mut data: WorldDataMut, mut behavior: WorldBehaviorBuilderMut, _render: &mut RenderFeatureRegistry) {
    data.resources_mut().insert(SceneLightResource::default());

    behavior
        .get_mut(Schedule::Start)
        .insert_system(load_scene_light_settings_system);

    let mut update = behavior.get_mut(Schedule::Update);

    update
        .group(SYSTEM_GROUP)
        .insert_child_system(click_scene_light_buttons_system)
        .insert_child_system(apply_scene_light_field_text_system)
        .insert_child_system(update_scene_light_panel_system);

    update
        .order_system(check_ui_interaction_system)
        .before_system(click_scene_light_buttons_system)
        .before_system(update_scene_light_panel_system);

    update
        .order_system(update_selected_input_field_text_system)
        .before_system(apply_scene_light_field_text_system);

    update
        .order_system(apply_input_field_context_menu_system)
        .before_system(apply_scene_light_field_text_system);

    update
        .order_system(click_scene_light_buttons_system)
        .before_system(update_scene_light_panel_system);

    update
        .order_system(click_scene_light_buttons_system)
        .before_system(render_scene_window_system);

    update
        .order_system(apply_scene_light_field_text_system)
        .before_system(render_scene_window_system);
}

#[derive(Serializable, Clone, Debug, PartialEq)]
pub struct SceneLightSettings {
    pub override_ambient_light: bool,
    pub ambient_light: Vec3<f32>,
    pub default_light_euler: Vec3<f32>,
    pub shadows: bool,
}

impl Default for SceneLightSettings {
    fn default() -> Self {
        Self {
            override_ambient_light: false,
            ambient_light: Vec3::splat(0.1),
            default_light_euler: Vec3::new(-140.0, 30.0, 0.0),
            shadows: true,
        }
    }
}

#[derive(Resource, Default)]
pub struct SceneLightResource {
    pub settings: SceneLightSettings,
    pub is_panel_open: bool,
    settings_path: Option<PathBuf>,
}

impl SceneLightResource {
    pub fn ambient_light(&self) -> Option<Vec3<f32>> {
        self.settings.override_ambient_light.then_some(self.settings.ambient_light)
    }

    pub fn default_light(&self) -> StandardLight {
        let euler = self.settings.default_light_euler.map(|v| (v as f64).to_radians());
        let rotation = Quat::<f32>::from_euler_angles(euler, RotationOrder3D::Zxy).to_matrix();

        StandardLight::Directional {
            color: DEFAULT_LIGHT_COLOR,
            direction_dst: (rotation * Vec3::new(0.0, -1.0, 0.0)).normalized(),
            shadows: self.settings.shadows,
        }
    }

    fn store(&self) {
        let Some(path) = &self.settings_path else {
            return;
        };

        let mut on_err = |err: SerializationError| eprintln!("failed to serialize scene light settings. {err}");
        let mut ctx = SerializerCtx::new(PureSerializerCtxState, &mut on_err);

        let json_str = serde_json::to_string_pretty(&ctx.serialize(&self.settings, "").to_json()).unwrap();

        if let Some(dir) = path.parent()
            && let Err(err) = std::fs::create_dir_all(dir)
        {
            eprintln!("failed to create {:?}. {}", dir, err);
            return;
        }

        if let Err(err) = std::fs::write(path, json_str.into_bytes()) {
            eprintln!("failed to write to {:?}. {}", path, err);
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SceneLightField {
    AmbientLight,
    DefaultLightEuler,
}

#[derive(Component, Default, Debug)]
pub struct SceneLightButtonComponent;

#[derive(Component, Default, Debug)]
pub struct SceneLightPanelComponent;

#[derive(Component, Default, Debug)]
pub struct SceneAmbientOverrideButtonComponent;

#[derive(Component, Default, Debug)]
pub struct SceneShadowsButtonComponent;

#[derive(Component, Debug)]
pub struct SceneLightFieldComponent {
    pub field: SceneLightField,
    pub axis: usize,
}

fn load_settings(path: &PathBuf) -> Option<SceneLightSettings> {
    let json_str = std::fs::read_to_string(path).ok()?;

    let json = match serde_json::from_str(&json_str) {
        Ok(json) => json,
        Err(err) => {
            eprintln!("failed to parse {:?}. {}", path, err);
            return None;
        }
    };

    let mut errors = Vec::new();
    let mut on_err = |err: SerializationError| errors.push(err.to_string());
    let mut ctx = SerializerCtx::new(PureSerializerCtxState, &mut on_err);

    let settings: SceneLightSettings = ctx.deserialize_default("", &SerializedValue::from_json(&json));

    if !errors.is_empty() {
        eprintln!("failed to deserialize {:?}. {}", path, errors.join("; "));
        return None;
    }

    Some(settings)
}

fn load_scene_light_settings_system(open_project: Res<OpenProjectResource>, mut light_res: ResMut<SceneLightResource>) {
    let path = PathBuf::from(&open_project.dir_path)
        .join(SETTINGS_DIR)
        .join(SETTINGS_FILE_NAME);

    light_res.settings = load_settings(&path).unwrap_or_default();
    light_res.settings_path = Some(path);
}

fn click_scene_light_buttons_system(
    light_button_q: WorldQuery<&ButtonComponent, WithFilter<SceneLightButtonComponent>>,
    override_button_q: WorldQuery<&ButtonComponent, WithFilter<SceneAmbientOverrideButtonComponent>>,
    shadows_button_q: WorldQuery<&ButtonComponent, WithFilter<SceneShadowsButtonComponent>>,
    mut light_res: ResMut<SceneLightResource>,
) {
    if light_button_q.iter().any(|button_c| button_c.was_clicked_this_frame) {
        light_res.is_panel_open = !light_res.is_panel_open;
    }

    if override_button_q.iter().any(|button_c| button_c.was_clicked_this_frame) {
        light_res.settings.override_ambient_light = !light_res.settings.override_ambient_light;
        light_res.store();
    }

    if shadows_button_q.iter().any(|button_c| button_c.was_clicked_this_frame) {
        light_res.settings.shadows = !light_res.settings.shadows;
        light_res.store();
    }
}

fn apply_scene_light_field_text_system(
    text_changed_evt: Evt<InputFieldTextChangedEvent>,
    field_q: WorldQuery<(&InputFieldComponent, &SceneLightFieldComponent)>,
    text_q: WorldQuery<&TextComponent>,
    mut light_res: ResMut<SceneLightResource>,
) {
    let mut is_changed = false;

    for evt in text_changed_evt.iter() {
        continue_if_not!(Some((input_field_c, field_c)) = field_q.get(evt.input_field));
        continue_if_not!(Some(text_c) = text_q.get(input_field_c.text));
        continue_if_not!(Ok(value) = text_c.text.as_str().trim().parse::<f32>());

        let vec = match field_c.field {
            SceneLightField::AmbientLight => &mut light_res.settings.ambient_light,
            SceneLightField::DefaultLightEuler => &mut light_res.settings.default_light_euler,
        };
        let slot = &mut vec.as_array_mut()[field_c.axis];

        if *slot != value {
            *slot = value;
            is_changed = true;
        }
    }

    if is_changed {
        light_res.store();
    }
}

fn update_scene_light_panel_system(
    mut ent: EntitiesHolderMut,
    light_res: Res<SceneLightResource>,
    assets_res: Res<StandardAssetsResource>,
) {
    let panels = ent
        .query_filtered::<EntityId, WithFilter<SceneLightPanelComponent>>()
        .iter()
        .collect::<Vec<_>>();
    let image = ent
        .query_filtered::<EntityId, WithFilter<SceneViewImageComponent>>()
        .iter()
        .next();

    if !light_res.is_panel_open {
        for ent_panel in panels {
            destroy_entity_and_children(ent.as_mut(), ent_panel);
        }
    } else if panels.is_empty()
        && let Some(ent_image) = image
    {
        spawn_scene_light_panel(ent.as_mut(), &assets_res, ent_image, &light_res.settings);
    }

    let buttons = [
        (
            ent.query_filtered::<EntityId, WithFilter<SceneLightButtonComponent>>()
                .iter()
                .collect::<Vec<_>>(),
            light_res.is_panel_open,
        ),
        (
            ent.query_filtered::<EntityId, WithFilter<SceneAmbientOverrideButtonComponent>>()
                .iter()
                .collect::<Vec<_>>(),
            light_res.settings.override_ambient_light,
        ),
        (
            ent.query_filtered::<EntityId, WithFilter<SceneShadowsButtonComponent>>()
                .iter()
                .collect::<Vec<_>>(),
            light_res.settings.shadows,
        ),
    ];

    for (ent_buttons, is_active) in buttons {
        for ent_button in ent_buttons {
            if let Some(image_c) = ent.get_component_mut::<ImageComponent>(ent_button) {
                image_c.color = if is_active { BUTTON_COLOR_ACTIVE } else { BUTTON_COLOR };
            }
        }
    }
}
