use fruits_ffi::{FfiOption, FfiString};
use fruits_window::{FullscreenState, WindowChangeRequests, WindowIcon, WindowState};
use winit::{dpi::PhysicalSize, window::{Fullscreen, Icon, Window}};

fn fullscreen_state_from_window(window: &Window) -> FullscreenState {
    match window.fullscreen() {
        Some(Fullscreen::Borderless { .. }) => FullscreenState::Borderless,
        Some(Fullscreen::Exclusive { .. }) => FullscreenState::Exclusive,
        None => match window.is_maximized() {
            true => FullscreenState::WindowedMaximized,
            false => FullscreenState::Windowed,
        },
    }
}

fn fullscreen_state_apply(fullscreen: FullscreenState, window: &Window) {
    match fullscreen {
        FullscreenState::Windowed => {
            window.set_maximized(false);
            window.set_fullscreen(None);
        },
        FullscreenState::WindowedMaximized => {
            window.set_maximized(true);
            window.set_fullscreen(None);
        },
        FullscreenState::Borderless => {
            window.set_maximized(false);
            window.set_fullscreen(Some(Fullscreen::Borderless(None)));
        },
        FullscreenState::Exclusive => {
            window.set_maximized(false);

            let fullscreen = if let Some(monitor) = window.current_monitor() && let Some(video_mode) = monitor.video_modes().next() {
                Fullscreen::Exclusive(video_mode)
            } else {
                Fullscreen::Borderless(None)
            };
            window.set_fullscreen(Some(fullscreen));
        },
    }
}

fn window_size_from_window(window: &Window) -> [u32; 2] {
    let size = window.inner_size();

    [size.width, size.height]
}

fn window_size_apply(size: [u32; 2], window: &Window) {
    _ = window.request_inner_size(winit::dpi::Size::Physical(PhysicalSize { width: size[0], height: size[1], }));
}

fn window_title_from_window(window: &Window) -> FfiString {
    window.title().into()
}

fn window_icon_to_winit(icon: &WindowIcon) -> Option<Icon> {
    let [width, height] = icon.size;

    Icon::from_rgba(icon.rgba.to_vec(), width, height)
        .inspect_err(|err| eprintln!("ayf: Failed to set window icon. {}", err))
        .ok()
}

pub(crate) fn window_state_from_window(window: &Window) -> WindowState {
    WindowState {
        fullscreen: fullscreen_state_from_window(window),
        size: window_size_from_window(window),
        title: window_title_from_window(window),
        is_resizable: window.is_resizable(),
        icon: FfiOption::None,
    }
}

pub(crate) fn window_state_refresh_from_window(state: &mut WindowState, window: &Window) {
    state.fullscreen = fullscreen_state_from_window(window);
    state.size = window_size_from_window(window);
}

pub(crate) fn window_state_apply_changes(state: &WindowState, changes: &WindowChangeRequests, window: &Window) {
    if let Some(fullscreen) = changes.fullscreen.as_ref().filter(|f| **f != state.fullscreen) {
        fullscreen_state_apply(*fullscreen, window);
    }
    if let Some(size) = changes.size.as_ref().filter(|s| **s != state.size) {
        window_size_apply(*size, window);
    }
    if let Some(title) = changes.title.as_ref().filter(|t| **t != state.title) {
        window.set_title(title);
    }
    if let Some(resizable) = changes.is_resizable.as_ref().filter(|r| **r != state.is_resizable) {
        window.set_resizable(*resizable);
    }
    if let Some(icon) = changes.icon.as_ref().filter(|i| **i != state.icon) {
        window.set_window_icon(icon.as_ref().and_then(window_icon_to_winit));
    }
}
