//! Access the clipboard.
use std::any::Any;
use std::path::PathBuf;
use std::sync::Arc;

use dnd::{DndAction, DndDestinationRectangle, DndSurface};
use mime::{AsMimeTypes, ClipboardStoreData};

use crate::widget::{self, Element};

/// A widget tree used to render a drag-and-drop icon.
#[derive(Debug)]
pub struct IconSurface<E> {
    /// The element rendered into the icon surface.
    pub element: E,
    /// The widget state associated with the element.
    pub state: widget::tree::State,
    /// The offset of the icon relative to the pointer.
    pub offset: crate::Vector,
}

/// A type-erased drag-and-drop icon surface.
pub type DynIconSurface = IconSurface<Box<dyn Any>>;

impl<T: 'static, R: 'static> IconSurface<Element<'static, (), T, R>> {
    /// Creates a new drag-and-drop icon surface.
    pub fn new(
        element: Element<'static, (), T, R>,
        state: widget::tree::State,
        offset: crate::Vector,
    ) -> Self {
        Self {
            element,
            state,
            offset,
        }
    }

    /// Erases the theme and renderer types of the icon element.
    pub fn upcast(self) -> DynIconSurface {
        IconSurface {
            element: Box::new(self.element),
            state: self.state,
            offset: self.offset,
        }
    }
}

impl DynIconSurface {
    /// Downcasts the icon element to its concrete theme and renderer types.
    pub fn downcast<T: 'static, R: 'static>(self) -> IconSurface<Element<'static, (), T, R>> {
        IconSurface {
            element: *self
                .element
                .downcast()
                .expect("drag-and-drop icon surface has invalid element type"),
            state: self.state,
            offset: self.offset,
        }
    }
}

/// The source of a drag-and-drop operation.
#[derive(Debug, Clone)]
pub enum DndSource {
    /// A widget is the source.
    Widget(widget::Id),
    /// A window surface is the source.
    Surface(crate::window::Id),
}

/// A collection of drag-and-drop destination rectangles.
#[derive(Debug, Clone, Default)]
pub struct DndDestinationRectangles {
    rectangles: Vec<DndDestinationRectangle>,
}

impl DndDestinationRectangles {
    /// Creates an empty collection.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an empty collection with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            rectangles: Vec::with_capacity(capacity),
        }
    }

    /// Adds a destination rectangle.
    pub fn push(&mut self, rectangle: DndDestinationRectangle) {
        self.rectangles.push(rectangle);
    }

    /// Appends destination rectangles from another vector.
    pub fn append(&mut self, other: &mut Vec<DndDestinationRectangle>) {
        self.rectangles.append(other);
    }

    /// Returns the rectangles, ordered for hit-testing.
    pub fn into_rectangles(mut self) -> Vec<DndDestinationRectangle> {
        self.rectangles.reverse();
        self.rectangles
    }
}

impl AsRef<[DndDestinationRectangle]> for DndDestinationRectangles {
    fn as_ref(&self) -> &[DndDestinationRectangle] {
        &self.rectangles
    }
}

/// The system clipboard target used for MIME-based data transfers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// The standard clipboard.
    Standard,
    /// The primary selection, available on X11 and Wayland.
    Primary,
}

/// A set of clipboard requests.
pub struct DndStart {
    /// Whether this is an internal drag-and-drop operation.
    pub internal: bool,
    /// The widget or surface that initiated the operation.
    pub source: Option<DndSource>,
    /// An optional custom drag icon.
    pub icon: Option<DynIconSurface>,
    /// The MIME data being dragged.
    pub content: Box<dyn AsMimeTypes + Send + 'static>,
    /// The supported drag-and-drop actions.
    pub actions: DndAction,
}

impl std::fmt::Debug for DndStart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DndStart")
            .field("internal", &self.internal)
            .field("source", &self.source)
            .field("has_icon", &self.icon.is_some())
            .field("actions", &self.actions)
            .finish_non_exhaustive()
    }
}

/// A trait for interacting with the clipboard in widgets.
pub trait Clipboard {
    /// Reads the current content of the [`Clipboard`] as text.
    fn read(&self, _kind: Kind) -> Option<String> {
        None
    }

    /// Writes the given text contents to the [`Clipboard`].
    fn write(&mut self, _kind: Kind, _contents: String) {}

    /// Reads data from the clipboard.
    fn read_data(&self, _kind: Kind, _mimes: Vec<String>) -> Option<(Vec<u8>, String)> {
        None
    }

    /// Writes data to the clipboard.
    fn write_data(
        &mut self,
        _kind: Kind,
        _contents: ClipboardStoreData<Box<dyn Send + Sync + 'static + AsMimeTypes>>,
    ) {
    }

    /// Registers DnD destinations.
    fn register_dnd_destination(
        &self,
        _surface: DndSurface,
        _rectangles: Vec<DndDestinationRectangle>,
    ) {
    }

    /// Set the final action for the DnD operation.
    fn set_action(&self, _action: DndAction) {}

    /// Starts a DnD operation.
    fn start_dnd(
        &mut self,
        _internal: bool,
        _source_surface: Option<DndSource>,
        _icon_surface: Option<DynIconSurface>,
        _content: Box<dyn AsMimeTypes + Send + 'static>,
        _actions: DndAction,
    ) {
    }

    /// Ends a DnD operation.
    fn end_dnd(&self) {}

    /// Peeks DnD data.
    fn peek_dnd(&self, _mime: String) -> Option<(Vec<u8>, String)> {
        None
    }

    /// Request window size.
    fn request_logical_window_size(&self, _width: f32, _height: f32) {}
}

/// A set of clipboard and drag-and-drop requests produced by widgets.
pub struct Requests {
    /// The read requests the runtime must fulfill.
    pub reads: Vec<Kind>,
    /// The content that must be written to the clipboard by the runtime,
    /// if any.
    pub write: Option<Content>,
    /// Drag-and-drop source requests for the runtime to execute.
    pub dnd: Vec<DndStart>,
    /// Requested logical window size.
    pub requested_logical_size: Option<(f32, f32)>,
}

impl std::fmt::Debug for Requests {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Requests")
            .field("reads", &self.reads)
            .field("write", &self.write)
            .field("dnd_count", &self.dnd.len())
            .finish()
    }
}

impl Requests {
    /// Creates a new empty set of clipboard requests.
    pub fn new() -> Self {
        Self {
            reads: Vec::new(),
            write: None,
            dnd: Vec::new(),
            requested_logical_size: None,
        }
    }

    /// Merges the current requests with others.
    pub fn merge(&mut self, other: &mut Self) {
        self.reads.append(&mut other.reads);
        self.write = other.write.take().or(self.write.take());
        self.dnd.append(&mut other.dnd);
        if other.requested_logical_size.is_some() {
            self.requested_logical_size = other.requested_logical_size;
        }
    }
}

impl Default for Requests {
    fn default() -> Self {
        Self::new()
    }
}

/// Starts a DnD operation.
pub fn start_dnd<T: 'static, R: 'static>(
    clipboard: &mut dyn Clipboard,
    internal: bool,
    source_surface: Option<DndSource>,
    icon_surface: Option<IconSurface<Element<'static, (), T, R>>>,
    content: Box<dyn AsMimeTypes + Send + 'static>,
    actions: DndAction,
) {
    clipboard.start_dnd(
        internal,
        source_surface,
        icon_surface.map(IconSurface::upcast),
        content,
        actions,
    );
}

/// A clipboard event.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    /// The clipboard was read.
    Read(Result<Arc<Content>, Error>),

    /// The clipboard was written.
    Written(Result<(), Error>),
}

/// Some clipboard content.
#[derive(Debug, Clone, PartialEq)]
#[allow(missing_docs)]
#[non_exhaustive]
pub enum Content {
    Text(String),
    Html(String),
    #[cfg(feature = "image")]
    Image(Image),
    Files(Vec<PathBuf>),
}

impl From<String> for Content {
    fn from(text: String) -> Self {
        Self::Text(text)
    }
}

#[cfg(feature = "image")]
impl From<Image> for Content {
    fn from(image: Image) -> Self {
        Self::Image(image)
    }
}

impl From<Vec<PathBuf>> for Content {
    fn from(files: Vec<PathBuf>) -> Self {
        Self::Files(files)
    }
}

/// The kind of some clipboard [`Content`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(missing_docs)]
pub enum Kind {
    Text,
    Html,
    #[cfg(feature = "image")]
    Image,
    Files,
    Standard,
    Primary,
}

/// A clipboard image.
#[cfg(feature = "image")]
#[derive(Debug, Clone, PartialEq)]
pub struct Image {
    /// The pixels of the image in RGBA format.
    pub rgba: crate::Bytes,

    /// The physical [`Size`](crate::Size) of the image.
    pub size: crate::Size<u32>,
}

/// A clipboard error.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The clipboard in the current environment is either not present or could not be accessed.
    ClipboardUnavailable,

    /// The native clipboard is not accessible due to being held by another party.
    ClipboardOccupied,

    /// The clipboard contents were not available in the requested format.
    ContentNotAvailable,

    /// The image or the text that was about the be transferred to/from the clipboard could not be
    /// converted to the appropriate format.
    ConversionFailure,

    /// Any error that doesn't fit the other error types.
    Unknown {
        /// A description only meant to help the developer that should not be relied on as a
        /// means to identify an error case during runtime.
        description: Arc<String>,
    },
}
