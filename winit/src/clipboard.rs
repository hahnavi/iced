//! Access the clipboard.
#![allow(missing_docs)]

use std::sync::Mutex;
use std::{any::Any, borrow::Cow};

use crate::Control;

use crate::core::clipboard::{Content, Error, Kind, Target};
use crate::core::clipboard::{DndSource, DynIconSurface};
use log::trace;
use std::sync::Arc;
use winit::dpi::LogicalSize;
use winit::window::{Window, WindowId};

use dnd::{DndAction, DndDestinationRectangle, DndSurface, Icon};
use window_clipboard::{
    dnd::DndProvider,
    mime::{self, ClipboardData, ClipboardStoreData},
};

/// A buffer for short-term storage and transfer within and between
/// applications.
pub struct Clipboard {
    state: State,
    pub(crate) requested_logical_size: Arc<Mutex<Option<LogicalSize<f32>>>>,
}

pub(crate) struct StartDnd {
    pub(crate) internal: bool,
    pub(crate) source_surface: Option<DndSource>,
    pub(crate) icon_surface: Option<DynIconSurface>,
    pub(crate) content: Box<dyn mime::AsMimeTypes + Send + 'static>,
    pub(crate) actions: DndAction,
}

enum State {
    Connected {
        clipboard: window_clipboard::Clipboard,
        sender: ControlSender,
        // Held until drop to satisfy the safety invariants of
        // `window_clipboard::Clipboard`.
        //
        // Note that the field ordering is load-bearing.
        #[allow(dead_code)]
        window: Arc<dyn Window>,
        queued_events: Vec<StartDnd>,
    },
    Unavailable,
}

fn clipboard_error(error: impl std::fmt::Display) -> Error {
    Error::Unknown {
        description: Arc::new(error.to_string()),
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ControlSender {
    pub(crate) sender: iced_futures::futures::channel::mpsc::UnboundedSender<crate::Control>,
    pub(crate) proxy: winit::event_loop::EventLoopProxy,
}

impl dnd::Sender<DndSurface> for ControlSender {
    fn send(
        &self,
        event: dnd::DndEvent<DndSurface>,
    ) -> Result<(), std::sync::mpsc::SendError<dnd::DndEvent<DndSurface>>> {
        let res = self
            .sender
            .unbounded_send(Control::Dnd(event))
            .map_err(|_err| {
                std::sync::mpsc::SendError(dnd::DndEvent::Offer(None, dnd::OfferEvent::Leave))
            });
        self.proxy.wake_up();
        res
    }
}

impl Clipboard {
    /// Creates a new [`Clipboard`] for the given window.
    pub(crate) fn connect(window: Arc<dyn Window>, sender: ControlSender) -> Clipboard {
        #[allow(unsafe_code)]
        let state = unsafe { window_clipboard::Clipboard::connect(window.as_ref()) }
            .ok()
            .map(|c| State::Connected {
                clipboard: c,
                sender: sender.clone(),
                window,
                queued_events: Vec::new(),
            })
            .unwrap_or(State::Unavailable);

        #[cfg(wayland_platform)]
        if let State::Connected { clipboard, .. } = &state {
            clipboard.init_dnd(Box::new(sender));
        }

        Clipboard {
            state,
            requested_logical_size: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn proxy(&self) -> Option<winit::event_loop::EventLoopProxy> {
        if let State::Connected {
            sender: ControlSender { proxy, .. },
            ..
        } = &self.state
        {
            Some(proxy.clone())
        } else {
            None
        }
    }

    /// Creates a new [`Clipboard`] that isn't associated with a window.
    /// This clipboard will never contain a copied value.
    pub fn unconnected() -> Clipboard {
        Clipboard {
            state: State::Unavailable,
            requested_logical_size: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn get_queued(&mut self) -> Vec<StartDnd> {
        match &mut self.state {
            State::Connected { queued_events, .. } => std::mem::take(queued_events),
            State::Unavailable => {
                log::error!("Invalid request for queued dnd events");
                Vec::<StartDnd>::new()
            }
        }
    }

    /// Reads a typed item from the clipboard.
    pub fn read(&self, kind: Kind, callback: impl FnOnce(Result<Content, Error>) + Send + 'static) {
        let result = match (&self.state, kind) {
            (State::Connected { clipboard, .. }, Kind::Text) => {
                clipboard.read().map(Content::Text).map_err(clipboard_error)
            }
            (State::Connected { clipboard, .. }, Kind::Html) => clipboard
                .read_raw(vec!["text/html".to_owned()])
                .and_then(|data| data.ok())
                .and_then(|(bytes, _)| String::from_utf8(bytes).ok())
                .map(Content::Html)
                .ok_or(Error::ContentNotAvailable),
            (State::Connected { clipboard, .. }, Kind::Files) => clipboard
                .read_raw(vec!["text/uri-list".to_owned()])
                .and_then(|data| data.ok())
                .and_then(|(bytes, _)| String::from_utf8(bytes).ok())
                .map(|files| {
                    Content::Files(
                        files
                            .lines()
                            .filter(|line| !line.is_empty() && !line.starts_with('#'))
                            .map(|line| {
                                std::path::PathBuf::from(
                                    line.strip_prefix("file://").unwrap_or(line),
                                )
                            })
                            .collect(),
                    )
                })
                .ok_or(Error::ContentNotAvailable),
            #[cfg(feature = "image")]
            (State::Connected { .. }, Kind::Image) => Err(Error::ContentNotAvailable),
            (State::Connected { .. }, _) => Err(Error::ContentNotAvailable),
            (State::Unavailable, _) => Err(Error::ClipboardUnavailable),
        };

        callback(result);
    }

    /// Writes typed content to the clipboard.
    pub fn write(
        &mut self,
        content: Content,
        callback: impl FnOnce(Result<(), Error>) + Send + 'static,
    ) {
        let result = match (&mut self.state, content) {
            (State::Connected { clipboard, .. }, Content::Text(text)) => {
                clipboard.write(text).map_err(clipboard_error)
            }
            (State::Unavailable, _) => Err(Error::ClipboardUnavailable),
            _ => Err(Error::ConversionFailure),
        };

        callback(result);
    }

    /// Returns the identifier of the window used to create the [`Clipboard`], if any.
    pub fn window_id(&self) -> Option<WindowId> {
        match &self.state {
            State::Connected { window, .. } => Some(window.id()),
            State::Unavailable => None,
        }
    }

    pub fn read_data(&self, kind: Target, mimes: Vec<String>) -> Option<(Vec<u8>, String)> {
        match (&self.state, kind) {
            (State::Connected { clipboard, .. }, Target::Standard) => {
                clipboard.read_raw(mimes).and_then(|res| res.ok())
            }
            (State::Connected { clipboard, .. }, Target::Primary) => {
                clipboard.read_primary_raw(mimes).and_then(|res| res.ok())
            }
            (State::Unavailable, _) => None,
        }
    }

    pub fn write_data(
        &mut self,
        kind: Target,
        contents: ClipboardStoreData<Box<dyn Send + Sync + 'static + mime::AsMimeTypes>>,
    ) {
        match (&mut self.state, kind) {
            (State::Connected { clipboard, .. }, Target::Standard) => {
                _ = clipboard.write_data(contents)
            }
            (State::Connected { clipboard, .. }, Target::Primary) => {
                _ = clipboard.write_primary_data(contents)
            }
            (State::Unavailable, _) => {}
        }
    }

    pub fn start_dnd(
        &mut self,
        internal: bool,
        source_surface: Option<DndSource>,
        icon_surface: Option<DynIconSurface>,
        content: Box<dyn mime::AsMimeTypes + Send + 'static>,
        actions: DndAction,
    ) {
        match &mut self.state {
            State::Connected {
                queued_events,
                sender,
                ..
            } => {
                log::debug!(
                    "clipboard::start_dnd queued internal={} source={:?}",
                    internal,
                    source_surface
                );
                _ = sender.sender.unbounded_send(crate::Control::StartDnd);
                queued_events.push(StartDnd {
                    internal,
                    source_surface,
                    icon_surface,
                    content,
                    actions,
                });
            }
            State::Unavailable => {}
        }
    }

    pub fn register_dnd_destination(
        &self,
        surface: DndSurface,
        rectangles: Vec<DndDestinationRectangle>,
    ) {
        match &self.state {
            State::Connected { clipboard, .. } => {
                trace!(
                    target: "iced::winit::clipboard",
                    "register destination surface={:?} count={}",
                    surface,
                    rectangles.len()
                );
                for rect in &rectangles {
                    trace!(
                        target: "iced::winit::clipboard",
                        "rect id={:?} bounds=({:.2},{:.2},{:.2},{:.2}) actions={:?} preferred={:?}",
                        rect.id,
                        rect.rectangle.x,
                        rect.rectangle.y,
                        rect.rectangle.width,
                        rect.rectangle.height,
                        rect.actions,
                        rect.preferred
                    );
                }
                _ = clipboard.register_dnd_destination(surface, rectangles)
            }
            State::Unavailable => {}
        }
    }

    pub fn end_dnd(&self) {
        match &self.state {
            State::Connected { clipboard, .. } => _ = clipboard.end_dnd(),
            State::Unavailable => {}
        }
    }

    pub fn peek_dnd(&self, mime: String) -> Option<(Vec<u8>, String)> {
        match &self.state {
            State::Connected { clipboard, .. } => clipboard
                .peek_offer::<ClipboardData>(Some(Cow::Owned(mime)))
                .ok()
                .map(|res| (res.0, res.1)),
            State::Unavailable => None,
        }
    }

    pub fn set_action(&self, action: DndAction) {
        match &self.state {
            State::Connected { clipboard, .. } => _ = clipboard.set_action(action),
            State::Unavailable => {}
        }
    }

    pub fn request_logical_window_size(&self, width: f32, height: f32) {
        let mut logical_size = self.requested_logical_size.lock().unwrap();
        *logical_size = Some(LogicalSize::new(width, height));
    }

    pub(crate) fn start_dnd_winit(
        &self,
        internal: bool,
        source_surface: DndSurface,
        icon_surface: Option<Icon>,
        content: Box<dyn mime::AsMimeTypes + Send + 'static>,
        actions: DndAction,
    ) {
        match &self.state {
            State::Connected { clipboard, .. } => {
                _ = clipboard.start_dnd(internal, source_surface, icon_surface, content, actions)
            }
            State::Unavailable => {}
        }
    }
}
