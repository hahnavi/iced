mod a11y_tree;
pub mod id;
mod node;
mod traits;

pub use a11y_tree::*;
pub use accesskit;
pub use id::*;
pub use node::*;
pub use traits::*;

#[cfg(all(feature = "accesskit_macos", target_os = "macos"))]
pub use accesskit_macos;
#[cfg(all(feature = "accesskit_windows", target_os = "windows"))]
pub use accesskit_windows;
#[cfg(feature = "accesskit_winit")]
pub use accesskit_winit;
