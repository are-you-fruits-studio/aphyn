use fruits_window::{FullscreenState, WindowState};
use winit::{dpi::PhysicalSize, window::{Fullscreen, Window}};

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

fn fullscreen_state_apply_difference(prev: &FullscreenState, next: &FullscreenState, window: &Window) {
    if prev == next {
        return;
    }

    match next {
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

fn window_size_apply_difference(prev: &[u32; 2], next: &[u32; 2], window: &Window) {
    if prev == next {
        return;
    }

    _ = window.request_inner_size(winit::dpi::Size::Physical(PhysicalSize { width: next[0], height: next[1], }));
}

pub(crate) fn window_state_from_window(window: &Window) -> WindowState {
    WindowState {
        fullscreen: fullscreen_state_from_window(window),
        size: window_size_from_window(window),
    }
}

pub(crate) fn window_state_apply_difference(prev: &WindowState, next: &WindowState, window: &Window) {
    fullscreen_state_apply_difference(&prev.fullscreen, &next.fullscreen, window);
    window_size_apply_difference(&prev.size, &next.size, window);
}
