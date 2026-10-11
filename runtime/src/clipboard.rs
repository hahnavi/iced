//! Access the clipboard.
use crate::core::clipboard::{Content, Error, Kind, Target};
use crate::futures::futures::channel::oneshot;
use crate::task::{self, Task};
use window_clipboard::mime::{AllowedMimeTypes, AsMimeTypes};

use std::path::PathBuf;
use std::sync::Arc;

/// A clipboard action to be performed by some [`Task`].
///
/// [`Task`]: crate::Task
pub enum Action {
    /// Read the clipboard and produce `String` with the result.
    Read {
        /// The [`Kind`] of [`Content`] to read.
        kind: Kind,
        /// The channel to send the read contents.
        channel: oneshot::Sender<Result<Content, Error>>,
    },

    /// Write the given contents to the clipboard.
    Write {
        /// The [`Content`] to be written.
        content: Content,

        /// The channel to send the write result.
        channel: oneshot::Sender<Result<(), Error>>,
    },

    /// Write the given contents to the clipboard.
    WriteData(Box<dyn AsMimeTypes + Send + Sync + 'static>, Target),

    #[allow(clippy::type_complexity)]
    /// Read the clipboard and produce `T` with the result.
    ReadData(
        Vec<String>,
        oneshot::Sender<Option<(Vec<u8>, String)>>,
        // Box<dyn Fn(Option<(Vec<u8>, String)>) -> T + Send + 'static>,
        Target,
    ),
}

impl std::fmt::Debug for Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read { channel: _, kind } => {
                write!(f, "Action::Read{kind:?}")
            }
            Self::Write {
                content: _,
                channel: _,
            } => {
                write!(f, "Action::Write")
            }
            Self::WriteData(_, target) => {
                write!(f, "Action::WriteData({target:?})")
            }
            Self::ReadData(_, _, target) => {
                write!(f, "Action::ReadData({target:?})")
            }
        }
    }
}

/// Read the given [`Kind`] of [`Content`] from the clipboard.
pub fn read(kind: Kind) -> Task<Result<Arc<Content>, Error>> {
    task::oneshot(|channel| crate::Action::Clipboard(Action::Read { kind, channel }))
        .map(|result| result.map(Arc::new))
}

/// Read the current text contents of the clipboard.
pub fn read_text() -> Task<Result<Arc<String>, Error>> {
    task::oneshot(|channel| {
        crate::Action::Clipboard(Action::Read {
            kind: Kind::Text,
            channel,
        })
    })
    .map(|result| {
        let Ok(Content::Text(text)) = result else {
            return Err(Error::ContentNotAvailable);
        };

        Ok(Arc::new(text))
    })
}

/// Read the current HTML contents of the clipboard.
pub fn read_html() -> Task<Result<Arc<String>, Error>> {
    task::oneshot(|channel| {
        crate::Action::Clipboard(Action::Read {
            kind: Kind::Html,
            channel,
        })
    })
    .map(|result| {
        let Ok(Content::Html(html)) = result else {
            return Err(Error::ContentNotAvailable);
        };

        Ok(Arc::new(html))
    })
}

/// Read the current file paths of the clipboard.
pub fn read_files() -> Task<Result<Arc<[PathBuf]>, Error>> {
    task::oneshot(|channel| {
        crate::Action::Clipboard(Action::Read {
            kind: Kind::Files,
            channel,
        })
    })
    .map(|result| {
        let Ok(Content::Files(files)) = result else {
            return Err(Error::ContentNotAvailable);
        };

        Ok(Arc::from(files))
    })
}

/// Read the current [`Image`](crate::core::clipboard::Image) of the clipboard.
#[cfg(feature = "image")]
pub fn read_image() -> Task<Result<crate::core::clipboard::Image, Error>> {
    task::oneshot(|channel| {
        crate::Action::Clipboard(Action::Read {
            kind: Kind::Image,
            channel,
        })
    })
    .map(|result| {
        let Ok(Content::Image(image)) = result else {
            return Err(Error::ContentNotAvailable);
        };

        Ok(image)
    })
}

/// Write the given [`Content`] to the clipboard.
pub fn write(content: impl Into<Content>) -> Task<Result<(), Error>> {
    let content = content.into();

    task::oneshot(|channel| crate::Action::Clipboard(Action::Write { content, channel }))
}
/// Read the current contents of the clipboard.
pub fn read_data<T: AllowedMimeTypes>() -> Task<Option<T>> {
    task::oneshot(|tx| {
        crate::Action::Clipboard(Action::ReadData(T::allowed().into(), tx, Target::Standard))
    })
    .map(|d| d.and_then(|d| T::try_from(d).ok()))
}

/// Write the given contents to the clipboard.
pub fn write_data<Message>(
    contents: impl AsMimeTypes + std::marker::Sync + std::marker::Send + 'static,
) -> Task<Message> {
    task::effect(crate::Action::Clipboard(Action::WriteData(
        Box::new(contents),
        Target::Standard,
    )))
}

/// Read from the primary clipboard
pub fn read_primary_data<T: AllowedMimeTypes>() -> Task<Option<T>> {
    task::oneshot(|tx| {
        crate::Action::Clipboard(Action::ReadData(
            T::allowed().into(),
            tx,
            // Box::new(move |d| f(d.and_then(|d| T::try_from(d).ok()))),
            Target::Primary,
        ))
    })
    .map(|d| d.and_then(|d| T::try_from(d).ok()))
}

/// Write the given contents to the clipboard.
pub fn write_primary_data<Message>(
    contents: impl AsMimeTypes + std::marker::Sync + std::marker::Send + 'static,
) -> Task<Message> {
    task::effect(crate::Action::Clipboard(Action::WriteData(
        Box::new(contents),
        Target::Primary,
    )))
}
