use std::path::PathBuf;

use fruits_engine::*;

use crate::{
    SYSTEM_GROUP,
    features::{
        test_window::{open_window_by_kind, open_windows_system},
        ui_window::{close_window_system, drag_window_system, rescale_window_system},
    },
    prefabs::WindowComponent,
};

const APP_NAME: &str = "fruits_editor";
const FILE_NAME: &str = "windows_layout.json";

pub fn register_feature(mut world: WorldBuilderMut) {
    let mut behavior = world.behavior_mut();

    {
        let mut start = behavior.get_mut(Schedule::Start);

        start.insert_system(restore_windows_layout_system);

        start.order_system(crate::init_system).before_system(restore_windows_layout_system);
    }

    {
        let mut update = behavior.get_mut(Schedule::Update);

        update.group(SYSTEM_GROUP).insert_child_system(save_windows_layout_system);

        update.order_system(open_windows_system).before_system(save_windows_layout_system);
        update.order_system(close_window_system).before_system(save_windows_layout_system);
        update.order_system(drag_window_system).before_system(save_windows_layout_system);
        update.order_system(rescale_window_system).before_system(save_windows_layout_system);
    }
}

#[derive(Serializable, Default, PartialEq, Clone, Debug)]
struct WindowLayoutEntry {
    kind: String,
    offset_x: f32,
    offset_y: f32,
    width: f32,
    height: f32,
}

#[derive(Serializable, Default, PartialEq, Clone, Debug)]
struct WindowsLayout {
    windows: Vec<WindowLayoutEntry>,
}

#[derive(Resource, Default)]
pub struct WindowsLayoutResource {
    last_saved: WindowsLayout,
}

fn layout_file_path() -> PathBuf {
    save_path(APP_NAME).join(FILE_NAME)
}

fn load_layout() -> Option<WindowsLayout> {
    let path = layout_file_path();

    let json_str = std::fs::read_to_string(&path).ok()?;

    let json = match serde_json::from_str(&json_str) {
        Ok(json) => json,
        Err(err) => {
            eprintln!("failed to parse {:?}. {}", &path, err);
            return None;
        }
    };

    let mut errors = Vec::new();
    let mut on_err = |err: SerializationError| errors.push(err.to_string());
    let mut ctx = SerializerCtx::new(PureSerializerCtxState, &mut on_err);

    let layout: WindowsLayout = ctx.deserialize_default("", &SerializedValue::from_json(&json));

    if !errors.is_empty() {
        eprintln!("failed to deserialize {:?}. {}", &path, errors.join("; "));
        return None;
    }

    Some(layout)
}

fn store_layout(layout: &WindowsLayout) {
    let mut on_err = |err: SerializationError| eprintln!("failed to serialize windows layout. {err}");
    let mut ctx = SerializerCtx::new(PureSerializerCtxState, &mut on_err);

    let json_str = serde_json::to_string_pretty(&ctx.serialize(layout, "").to_json()).unwrap();

    let path = layout_file_path();

    if let Some(dir) = path.parent()
        && let Err(err) = std::fs::create_dir_all(dir)
    {
        eprintln!("failed to create {:?}. {}", dir, err);
        return;
    }

    if let Err(err) = std::fs::write(&path, json_str.into_bytes()) {
        eprintln!("failed to write to {:?}. {}", &path, err);
    }
}

fn px_or_zero(val: UiVal) -> f32 {
    match val {
        UiVal { unit: UiUnit::Px, val } => val,
        _ => 0.0,
    }
}

fn restore_windows_layout_system(mut world: WorldDataMut) {
    let layout = load_layout().unwrap_or_default();

    for entry in &layout.windows {
        let Some(ent_window) = open_window_by_kind(world.as_mut(), &entry.kind) else {
            continue;
        };

        let mut ent = world.entities_mut();

        let Some(rect_c) = ent.get_component_mut::<LocalRectComponent>(ent_window) else {
            continue;
        };

        rect_c.offset = Vec2::new(UiVal::px(entry.offset_x), UiVal::px(entry.offset_y));
        rect_c.scale = Vec2::new(UiVal::px(entry.width).into(), UiVal::px(entry.height).into());
    }

    world.resources_mut().insert(WindowsLayoutResource { last_saved: layout });
}

fn save_windows_layout_system(
    input_res: Res<InputResource>,
    mut layout_res: ResMut<WindowsLayoutResource>,
    window_q: WorldQuery<(&LocalRectComponent, &WindowComponent)>,
    title_q: WorldQuery<&TextComponent>,
) {
    if input_res.mouse.is_pressed(MouseButton::Left) {
        return;
    }

    let mut windows = window_q
        .iter()
        .filter_map(|(rect_c, window_c)| {
            let title_c = title_q.get(window_c.title)?;

            Some(WindowLayoutEntry {
                kind: title_c.text.to_string(),
                offset_x: px_or_zero(rect_c.offset.x),
                offset_y: px_or_zero(rect_c.offset.y),
                width: rect_c.scale.x.into_option().map(px_or_zero).unwrap_or(0.0),
                height: rect_c.scale.y.into_option().map(px_or_zero).unwrap_or(0.0),
            })
        })
        .collect::<Vec<_>>();

    windows.sort_by(|a, b| a.kind.cmp(&b.kind));

    let layout = WindowsLayout { windows };

    if layout == layout_res.last_saved {
        return;
    }

    store_layout(&layout);

    layout_res.last_saved = layout;
}
