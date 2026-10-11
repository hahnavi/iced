//! Configure the window of your application in native platforms.

pub mod icon;

pub use icon::Icon;

pub use settings::{PlatformSpecific, Settings};

pub use crate::core::window::*;
pub use crate::runtime::window::*;
