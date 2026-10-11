use cctk::{
    sctk::shell::wlr_layer::Layer,
    wayland_protocols::ext::foreign_toplevel_list::v1::client::ext_foreign_toplevel_handle_v1::ExtForeignToplevelHandleV1,
};

/// A change to the set of surfaces overlapping a Wayland surface.
#[derive(Debug, Clone, PartialEq)]
pub enum OverlapNotifyEvent {
    /// A toplevel surface began overlapping.
    OverlapToplevelAdd {
        /// The foreign handle of the toplevel.
        toplevel: ExtForeignToplevelHandleV1,
        /// Its rectangle in logical coordinates.
        logical_rect: crate::Rectangle,
    },
    /// A toplevel surface stopped overlapping.
    OverlapToplevelRemove {
        /// The foreign handle of the toplevel.
        toplevel: ExtForeignToplevelHandleV1,
    },
    /// A layer surface began overlapping.
    OverlapLayerAdd {
        /// The identifier of the layer surface.
        identifier: String,
        /// The layer surface namespace.
        namespace: String,
        /// The exclusive zone of the layer surface.
        exclusive: u32,
        /// The layer of the surface, if known.
        layer: Option<Layer>,
        /// Its rectangle in logical coordinates.
        logical_rect: crate::Rectangle,
    },
    /// A layer surface stopped overlapping.
    OverlapLayerRemove {
        /// The identifier of the layer surface.
        identifier: String,
    },
}
