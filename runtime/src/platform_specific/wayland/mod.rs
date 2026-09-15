//! Wayland specific actions

use std::fmt::Debug;

use iced_core::{Rectangle, window::Id};

use crate::oneshot;

use cctk::sctk::reexports::client::protocol::wl_surface::WlSurface;

/// activation Actions
pub mod activation;

/// layer surface actions
pub mod layer_surface;
/// popup actions
pub mod popup;
/// session locks
pub mod session_lock;

// subsurfaces
pub mod subsurface;

/// Platform specific actions defined for wayland
pub enum Action {
    /// LayerSurface Actions
    LayerSurface(layer_surface::Action),
    /// popup
    Popup(popup::Action),
    /// activation
    Activation(activation::Action),
    /// session lock
    SessionLock(session_lock::Action),
    /// Overlap Notify
    OverlapNotify(Id, bool),
    /// Subsurfaces
    Subsurface(subsurface::Action),
    /// Keyboard inhibit shortcuts
    InhibitShortcuts(bool),
    /// Rounded corners in logical space
    RoundedCorners(iced_core::window::Id, Option<CornerRadius>),
    /// Blur effect for a surface
    BlurSurface(Id, Option<Vec<Rectangle>>),
    /// Sets the parent toplevel of a window from an xdg-foreign handle.
    SetWindowParent(Id, Option<String>),
    /// Marks a window as a dialog and optionally modal (xdg-dialog-v1).
    SetWindowModal(Id, bool),
    /// Requests the Wayland surface of a window.
    WindowSurface(Id, oneshot::Sender<Option<WlSurface>>),
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CornerRadius {
    pub top_left: u32,
    pub top_right: u32,
    pub bottom_left: u32,
    pub bottom_right: u32,
}

impl Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Action::LayerSurface(arg0) => {
                f.debug_tuple("LayerSurface").field(arg0).finish()
            }
            Action::Popup(arg0) => f.debug_tuple("Popup").field(arg0).finish(),
            Action::Activation(arg0) => {
                f.debug_tuple("Activation").field(arg0).finish()
            }
            Action::SessionLock(arg0) => {
                f.debug_tuple("SessionLock").field(arg0).finish()
            }
            Action::OverlapNotify(id, _) => {
                f.debug_tuple("OverlapNotify").field(id).finish()
            }
            Action::Subsurface(action) => {
                f.debug_tuple("Subsurface").field(action).finish()
            }
            Action::InhibitShortcuts(v) => {
                f.debug_tuple("InhibitShortcuts").field(v).finish()
            }
            Action::RoundedCorners(id, v) => {
                f.debug_tuple("RoundedCorners").field(id).field(v).finish()
            }
            Action::BlurSurface(id, rectangles) => f
                .debug_tuple("BlurSurface")
                .field(id)
                .field(rectangles)
                .finish(),
            Action::SetWindowParent(id, _) => {
                f.debug_tuple("SetWindowParent").field(id).finish()
            }
            Action::SetWindowModal(id, modal) => f
                .debug_tuple("SetWindowModal")
                .field(id)
                .field(modal)
                .finish(),
            Action::WindowSurface(id, _) => {
                f.debug_tuple("WindowSurface").field(id).finish()
            }
        }
    }
}
