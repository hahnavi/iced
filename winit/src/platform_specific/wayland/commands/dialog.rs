//! Sets the dialog relationship of a window.
//!
//! The parent is imported with `xdg-foreign` and the modal state with
//! `xdg-dialog`.

use crate::core::window::Id as SurfaceId;
use iced_runtime::{
    self, Action, Task,
    platform_specific::{self, wayland},
    task,
};
use wayland_client::protocol::wl_surface::WlSurface;

/// Imports the toplevel with the given xdg-foreign handle as the parent of the
/// window.
pub fn set_parent<Message>(
    window: SurfaceId,
    parent: Option<String>,
) -> Task<Message> {
    task::effect(Action::PlatformSpecific(
        platform_specific::Action::Wayland(wayland::Action::SetWindowParent(
            window, parent,
        )),
    ))
}

/// Marks the window as a dialog and, if `modal` is set, blocks its parent until
/// the window is closed.
pub fn set_modal<Message>(window: SurfaceId, modal: bool) -> Task<Message> {
    task::effect(Action::PlatformSpecific(
        platform_specific::Action::Wayland(wayland::Action::SetWindowModal(
            window, modal,
        )),
    ))
}

/// Returns the Wayland surface of the window.
pub fn window_surface(window: SurfaceId) -> Task<Option<WlSurface>> {
    task::oneshot(|channel| {
        Action::PlatformSpecific(platform_specific::Action::Wayland(
            wayland::Action::WindowSurface(window, channel),
        ))
    })
}
