//! Scrollables let users navigate an endless amount of content with a scrollbar.
//!
//! # Example
//! ```no_run
//! # mod iced { pub mod widget { pub use iced_widget::*; } }
//! # pub type State = ();
//! # pub type Element<'a, Message> = iced_widget::core::Element<'a, Message, iced_widget::Theme, iced_widget::Renderer>;
//! use iced::widget::{column, scrollable, space};
//!
//! enum Message {
//!     // ...
//! }
//!
//! fn view(state: &State) -> Element<'_, Message> {
//!     scrollable(column![
//!         "Scroll me!",
//!         space().height(3000),
//!         "You did it!",
//!     ]).into()
//! }
//! ```
use crate::container;
use crate::core::alignment;
use crate::core::border::{self, Border};
use crate::core::clipboard::DndDestinationRectangles;
use iced_runtime::core::widget::Id;
#[cfg(feature = "a11y")]
use std::borrow::Cow;

use crate::core::event;
use crate::core::keyboard;
use crate::core::layout;
use crate::core::mouse;
use crate::core::overlay;
use crate::core::renderer;
use crate::core::text;
use crate::core::time::{Duration, Instant};
use crate::core::touch;
use crate::core::widget::operation::{self, Operation};
use crate::core::widget::tree::{self, Tree};
use crate::core::window;
use crate::core::{
    self, Background, Clipboard, Color, Element, Event, InputMethod, Layout,
    Length, Padding, Pixels, Point, Rectangle, Shadow, Shell, Size, Theme,
    Vector, Widget, id::Internal,
};

use iced_runtime::{Action, Task, task};
pub use operation::scrollable::{AbsoluteOffset, RelativeOffset};

/// The scroll step, in logical pixels, of a single wheel detent for a
/// viewport of the given size.
///
/// Scales with `page_size^(2/3)`, so larger viewports scroll farther per
/// detent.
fn wheel_step(page_size: f32) -> f32 {
    page_size.max(1.0).powf(2.0 / 3.0)
}

/// Multiplier applied to precise (touchpad) scroll deltas, so a finger
/// movement covers a comfortable amount of content.
const PRECISE_SCROLL_SCALE: f32 = 2.5;

/// Time constant, in seconds, of the exponential interpolation used to
/// animate wheel scrolling. Smaller values are snappier.
const SMOOTHING_TIME_CONSTANT: f32 = 0.05;

/// Time constant, in seconds, of the momentum decay that follows a precise
/// (touchpad) scroll gesture.
const MOMENTUM_TIME_CONSTANT: f32 = 0.3;

/// Minimum velocity, in logical pixels per second, required to start a
/// momentum glide once a precise scroll gesture ends.
const MOMENTUM_MIN_VELOCITY: f32 = 45.0;

/// Maximum time between the last precise scroll input and the end of the
/// gesture for a momentum glide to start. Momentum never starts once the
/// content has been resting, so stopping the fingers stops the content.
const MOMENTUM_MAX_LAG: Duration = Duration::from_millis(120);

/// Time constant, in seconds, with which velocity is discounted by how long
/// before the end of the gesture the last precise input happened.
const MOMENTUM_LAG_DECAY: f32 = 0.08;

/// Maximum amount of time, in seconds, accounted for by a single animation
/// frame. Prevents jumps after a long frame.
const MAX_FRAME_DELTA: f32 = 0.05;

/// Maximum velocity, in logical pixels per second, tracked for momentum.
const MAX_SCROLL_VELOCITY: f32 = 8000.0;

/// Distance, in logical pixels, below which an animation snaps to its target.
const SNAP_DISTANCE: f32 = 0.25;

/// A widget that can vertically display an infinite amount of content with a
/// scrollbar.
///
/// # Example
/// ```no_run
/// # mod iced { pub mod widget { pub use iced_widget::*; } }
/// # pub type State = ();
/// # pub type Element<'a, Message> = iced_widget::core::Element<'a, Message, iced_widget::Theme, iced_widget::Renderer>;
/// use iced::widget::{column, scrollable, space};
///
/// enum Message {
///     // ...
/// }
///
/// fn view(state: &State) -> Element<'_, Message> {
///     scrollable(column![
///         "Scroll me!",
///         space().height(3000),
///         "You did it!",
///     ]).into()
/// }
/// ```
pub struct Scrollable<
    'a,
    Message,
    Theme = crate::Theme,
    Renderer = crate::Renderer,
> where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    id: Id,
    scrollbar_id: Id,
    #[cfg(feature = "a11y")]
    name: Option<Cow<'a, str>>,
    #[cfg(feature = "a11y")]
    description: Option<iced_accessibility::Description<'a>>,
    #[cfg(feature = "a11y")]
    label: Option<Vec<iced_accessibility::accesskit::NodeId>>,
    width: Length,
    height: Length,
    direction: Direction,
    auto_scroll: bool,
    content: Element<'a, Message, Theme, Renderer>,
    on_scroll: Option<Box<dyn Fn(Viewport) -> Message + 'a>>,
    class: Theme::Class<'a>,
    last_status: Option<Status>,
}

impl<'a, Message, Theme, Renderer> Scrollable<'a, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    /// Creates a new vertical [`Scrollable`].
    pub fn new(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        Self::with_direction(content, Direction::default())
    }

    /// Creates a new [`Scrollable`] with the given [`Direction`].
    pub fn with_direction(
        content: impl Into<Element<'a, Message, Theme, Renderer>>,
        direction: impl Into<Direction>,
    ) -> Self {
        Scrollable {
            id: Id::unique(),
            scrollbar_id: Id::unique(),
            #[cfg(feature = "a11y")]
            name: None,
            #[cfg(feature = "a11y")]
            description: None,
            #[cfg(feature = "a11y")]
            label: None,
            width: Length::Shrink,
            height: Length::Shrink,
            direction: direction.into(),
            auto_scroll: false,
            content: content.into(),
            on_scroll: None,
            class: Theme::default(),
            last_status: None,
        }
        .enclose()
    }

    fn enclose(mut self) -> Self {
        let size_hint = self.content.as_widget().size_hint();

        if self.direction.horizontal().is_none() {
            self.width = self.width.enclose(size_hint.width);
        }

        if self.direction.vertical().is_none() {
            self.height = self.height.enclose(size_hint.height);
        }

        self
    }

    /// Makes the [`Scrollable`] scroll horizontally, with default [`Scrollbar`] settings.
    pub fn horizontal(self) -> Self {
        self.direction(Direction::Horizontal(Scrollbar::default()))
    }

    /// Sets the [`Direction`] of the [`Scrollable`].
    pub fn direction(mut self, direction: impl Into<Direction>) -> Self {
        self.direction = direction.into();
        self.enclose()
    }

    /// Sets the [`widget::Id`] of the [`Scrollable`].
    pub fn id(mut self, id: impl Into<core::widget::Id>) -> Self {
        self.id = id.into();
        self
    }

    /// Sets the width of the [`Scrollable`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`Scrollable`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets a function to call when the [`Scrollable`] is scrolled.
    ///
    /// The function takes the [`Viewport`] of the [`Scrollable`]
    pub fn on_scroll(mut self, f: impl Fn(Viewport) -> Message + 'a) -> Self {
        self.on_scroll = Some(Box::new(f));
        self
    }

    /// Anchors the vertical [`Scrollable`] direction to the top.
    pub fn anchor_top(self) -> Self {
        self.anchor_y(Anchor::Start)
    }

    /// Anchors the vertical [`Scrollable`] direction to the bottom.
    pub fn anchor_bottom(self) -> Self {
        self.anchor_y(Anchor::End)
    }

    /// Anchors the horizontal [`Scrollable`] direction to the left.
    pub fn anchor_left(self) -> Self {
        self.anchor_x(Anchor::Start)
    }

    /// Anchors the horizontal [`Scrollable`] direction to the right.
    pub fn anchor_right(self) -> Self {
        self.anchor_x(Anchor::End)
    }

    /// Sets the [`Anchor`] of the horizontal direction of the [`Scrollable`], if applicable.
    pub fn anchor_x(mut self, alignment: Anchor) -> Self {
        match &mut self.direction {
            Direction::Horizontal(horizontal)
            | Direction::Both { horizontal, .. } => {
                horizontal.alignment = alignment;
            }
            Direction::Vertical { .. } => {}
        }

        self
    }

    /// Sets the [`Anchor`] of the vertical direction of the [`Scrollable`], if applicable.
    pub fn anchor_y(mut self, alignment: Anchor) -> Self {
        match &mut self.direction {
            Direction::Vertical(vertical)
            | Direction::Both { vertical, .. } => {
                vertical.alignment = alignment;
            }
            Direction::Horizontal { .. } => {}
        }

        self
    }

    /// Embeds the [`Scrollbar`] into the [`Scrollable`], instead of floating on top of the
    /// content.
    ///
    /// The `spacing` provided will be used as space between the [`Scrollbar`] and the contents
    /// of the [`Scrollable`].
    pub fn spacing(mut self, new_spacing: impl Into<Pixels>) -> Self {
        match &mut self.direction {
            Direction::Horizontal(scrollbar)
            | Direction::Vertical(scrollbar) => {
                scrollbar.spacing = Some(new_spacing.into().0);
            }
            Direction::Both { .. } => {}
        }

        self
    }

    /// Sets whether the user should be allowed to auto-scroll the [`Scrollable`]
    /// with the middle mouse button.
    ///
    /// By default, it is disabled.
    pub fn auto_scroll(mut self, auto_scroll: bool) -> Self {
        self.auto_scroll = auto_scroll;
        self
    }

    /// Sets the scrollbar width of the [`Scrollbar`].
    pub fn scrollbar_width(mut self, width: impl Into<Pixels>) -> Self {
        let width = width.into().0.max(0.0);

        match &mut self.direction {
            Direction::Horizontal(scrollbar)
            | Direction::Vertical(scrollbar) => {
                scrollbar.width = width;
            }
            Direction::Both {
                horizontal,
                vertical,
            } => {
                horizontal.width = width;
                vertical.width = width;
            }
        }

        self
    }

    /// Sets the scroller width of the [`Scrollbar`].
    pub fn scroller_width(mut self, width: impl Into<Pixels>) -> Self {
        let width = width.into().0.max(0.0);

        match &mut self.direction {
            Direction::Horizontal(scrollbar)
            | Direction::Vertical(scrollbar) => {
                scrollbar.scroller_width = width;
            }
            Direction::Both {
                horizontal,
                vertical,
            } => {
                horizontal.scroller_width = width;
                vertical.scroller_width = width;
            }
        }

        self
    }

    /// Sets the padding at the start and end of the [`Scrollbar`].
    pub fn scrollbar_padding(mut self, padding: impl Into<Pixels>) -> Self {
        let padding = padding.into().0.max(0.0);

        match &mut self.direction {
            Direction::Horizontal(scrollbar)
            | Direction::Vertical(scrollbar) => {
                scrollbar.padding = padding;
            }
            Direction::Both {
                horizontal,
                vertical,
            } => {
                horizontal.padding = padding;
                vertical.padding = padding;
            }
        }

        self
    }

    /// Sets the style of this [`Scrollable`].
    #[must_use]
    pub fn style(mut self, style: impl Fn(&Theme, Status) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the [`Scrollable`].
    #[cfg(feature = "advanced")]
    #[must_use]
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }

    #[cfg(feature = "a11y")]
    /// Sets the name of the [`Scrollable`].
    pub fn name(mut self, name: impl Into<Cow<'a, str>>) -> Self {
        self.name = Some(name.into());
        self
    }

    #[cfg(feature = "a11y")]
    /// Sets the description of the [`Scrollable`].
    pub fn description_widget(
        mut self,
        description: &impl iced_accessibility::Describes,
    ) -> Self {
        self.description = Some(iced_accessibility::Description::Id(
            description.description(),
        ));
        self
    }

    #[cfg(feature = "a11y")]
    /// Sets the description of the [`Scrollable`].
    pub fn description(mut self, description: impl Into<Cow<'a, str>>) -> Self {
        self.description =
            Some(iced_accessibility::Description::Text(description.into()));
        self
    }

    #[cfg(feature = "a11y")]
    /// Sets the label of the [`Scrollable`].
    pub fn label(mut self, label: &dyn iced_accessibility::Labels) -> Self {
        self.label =
            Some(label.label().into_iter().map(|l| l.into()).collect());
        self
    }
}

/// The direction of [`Scrollable`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    /// Vertical scrolling
    Vertical(Scrollbar),
    /// Horizontal scrolling
    Horizontal(Scrollbar),
    /// Both vertical and horizontal scrolling
    Both {
        /// The properties of the vertical scrollbar.
        vertical: Scrollbar,
        /// The properties of the horizontal scrollbar.
        horizontal: Scrollbar,
    },
}

impl Direction {
    /// Returns the horizontal [`Scrollbar`], if any.
    pub fn horizontal(&self) -> Option<&Scrollbar> {
        match self {
            Self::Horizontal(scrollbar) => Some(scrollbar),
            Self::Both { horizontal, .. } => Some(horizontal),
            Self::Vertical(_) => None,
        }
    }

    /// Returns the vertical [`Scrollbar`], if any.
    pub fn vertical(&self) -> Option<&Scrollbar> {
        match self {
            Self::Vertical(scrollbar) => Some(scrollbar),
            Self::Both { vertical, .. } => Some(vertical),
            Self::Horizontal(_) => None,
        }
    }

    fn align(&self, delta: Vector) -> Vector {
        let horizontal_alignment =
            self.horizontal().map(|p| p.alignment).unwrap_or_default();

        let vertical_alignment =
            self.vertical().map(|p| p.alignment).unwrap_or_default();

        let align = |alignment: Anchor, delta: f32| match alignment {
            Anchor::Start => delta,
            Anchor::End => -delta,
        };

        Vector::new(
            align(horizontal_alignment, delta.x),
            align(vertical_alignment, delta.y),
        )
    }
}

impl Default for Direction {
    fn default() -> Self {
        Self::Vertical(Scrollbar::default())
    }
}

/// A scrollbar within a [`Scrollable`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scrollbar {
    width: f32,
    margin: f32,
    scroller_width: f32,
    alignment: Anchor,
    spacing: Option<f32>,
    padding: f32,
}

impl Default for Scrollbar {
    fn default() -> Self {
        Self {
            width: 10.0,
            margin: 0.0,
            scroller_width: 10.0,
            alignment: Anchor::Start,
            spacing: None,
            padding: 0.0,
        }
    }
}

impl Scrollbar {
    /// Creates new [`Scrollbar`] for use in a [`Scrollable`].
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a [`Scrollbar`] with zero width to allow a [`Scrollable`] to scroll without a visible
    /// scroller.
    pub fn hidden() -> Self {
        Self::default().width(0).scroller_width(0)
    }

    /// Sets the scrollbar width of the [`Scrollbar`] .
    pub fn width(mut self, width: impl Into<Pixels>) -> Self {
        self.width = width.into().0.max(0.0);
        self
    }

    /// Sets the scrollbar margin of the [`Scrollbar`] .
    pub fn margin(mut self, margin: impl Into<Pixels>) -> Self {
        self.margin = margin.into().0;
        self
    }

    /// Sets the scroller width of the [`Scrollbar`] .
    pub fn scroller_width(mut self, scroller_width: impl Into<Pixels>) -> Self {
        self.scroller_width = scroller_width.into().0.max(0.0);
        self
    }

    /// Sets the [`Anchor`] of the [`Scrollbar`] .
    pub fn anchor(mut self, alignment: Anchor) -> Self {
        self.alignment = alignment;
        self
    }

    /// Sets whether the [`Scrollbar`] should be embedded in the [`Scrollable`], using
    /// the given spacing between itself and the contents.
    ///
    /// An embedded [`Scrollbar`] will always be displayed, will take layout space,
    /// and will not float over the contents.
    pub fn spacing(mut self, spacing: impl Into<Pixels>) -> Self {
        self.spacing = Some(spacing.into().0);
        self
    }

    /// Sets the padding at the start and end of the [`Scrollbar`].
    pub fn padding(mut self, padding: impl Into<Pixels>) -> Self {
        self.padding = padding.into().0.max(0.0);
        self
    }
}

/// The anchor of the scroller of the [`Scrollable`] relative to its [`Viewport`]
/// on a given axis.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Anchor {
    /// Scroller is anchoer to the start of the [`Viewport`].
    #[default]
    Start,
    /// Content is aligned to the end of the [`Viewport`].
    End,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Scrollable<'_, Message, Theme, Renderer>
where
    Theme: Catalog,
    Renderer: text::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::new())
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.content));
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let mut layout = |right_padding, bottom_padding| {
            layout::padded(
                limits,
                self.width,
                self.height,
                Padding {
                    right: right_padding,
                    bottom: bottom_padding,
                    ..Padding::ZERO
                },
                |limits| {
                    let is_horizontal = self.direction.horizontal().is_some();
                    let is_vertical = self.direction.vertical().is_some();

                    let child_limits = layout::Limits::with_compression(
                        limits.min(),
                        Size::new(
                            if is_horizontal {
                                f32::INFINITY
                            } else {
                                limits.max().width
                            },
                            if is_vertical {
                                f32::INFINITY
                            } else {
                                limits.max().height
                            },
                        ),
                        Size::new(is_horizontal, is_vertical),
                    );

                    self.content.as_widget_mut().layout(
                        &mut tree.children[0],
                        renderer,
                        &child_limits,
                    )
                },
            )
        };

        match self.direction {
            Direction::Vertical(Scrollbar {
                width,
                margin,
                spacing: Some(spacing),
                ..
            })
            | Direction::Horizontal(Scrollbar {
                width,
                margin,
                spacing: Some(spacing),
                ..
            }) => {
                let is_vertical =
                    matches!(self.direction, Direction::Vertical(_));

                let padding = width + margin * 2.0 + spacing;
                let state = tree.state.downcast_mut::<State>();

                let status_quo = layout(
                    if is_vertical && state.is_scrollbar_visible {
                        padding
                    } else {
                        0.0
                    },
                    if !is_vertical && state.is_scrollbar_visible {
                        padding
                    } else {
                        0.0
                    },
                );

                let is_scrollbar_visible = if is_vertical {
                    status_quo.children()[0].size().height
                        > status_quo.size().height
                } else {
                    status_quo.children()[0].size().width
                        > status_quo.size().width
                };

                if state.is_scrollbar_visible == is_scrollbar_visible {
                    status_quo
                } else {
                    log::trace!("Scrollbar status quo has changed");
                    state.is_scrollbar_visible = is_scrollbar_visible;

                    layout(
                        if is_vertical && state.is_scrollbar_visible {
                            padding
                        } else {
                            0.0
                        },
                        if !is_vertical && state.is_scrollbar_visible {
                            padding
                        } else {
                            0.0
                        },
                    )
                }
            }
            _ => layout(0.0, 0.0),
        }
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_mut::<State>();

        let bounds = layout.bounds();
        let content_layout = layout.children().next().unwrap();
        let content_bounds = content_layout.bounds();
        let translation =
            state.translation(self.direction, bounds, content_bounds);

        operation.pre_operation(Some(&self.id));

        operation.traverse(&mut |operation| {
            self.content.as_widget_mut().operate(
                &mut tree.children[0],
                layout
                    .children()
                    .next()
                    .unwrap()
                    .with_virtual_offset(translation + layout.virtual_offset()),
                renderer,
                operation,
            );
        });
        // XXX must be done after traversing to perform DFS
        operation.scrollable(
            Some(&self.id),
            bounds,
            content_bounds,
            translation,
            state,
        );
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        const AUTOSCROLL_DEADZONE: f32 = 20.0;
        const AUTOSCROLL_SMOOTHNESS: f32 = 1.5;

        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();
        let cursor_over_scrollable = cursor.position_over(bounds);

        let content = layout.children().next().unwrap();
        let content_bounds = content.bounds();

        let scrollbars =
            Scrollbars::new(state, self.direction, bounds, content_bounds);

        let (mouse_over_y_scrollbar, mouse_over_x_scrollbar) =
            scrollbars.is_mouse_over(cursor);

        let last_offsets = (state.offset_x, state.offset_y);

        if let Some(last_scrolled) = state.last_scrolled {
            let clear_transaction = match event {
                Event::Mouse(
                    mouse::Event::ButtonPressed(_)
                    | mouse::Event::ButtonReleased(_)
                    | mouse::Event::CursorLeft,
                ) => true,
                Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                    last_scrolled.elapsed() > Duration::from_millis(100)
                }
                _ => last_scrolled.elapsed() > Duration::from_millis(1500),
            };

            if clear_transaction {
                state.last_scrolled = None;
            }
        }

        let mut update = || {
            if let Some(scroller_grabbed_at) = state.y_scroller_grabbed_at() {
                match event {
                    Event::Mouse(mouse::Event::CursorMoved { .. })
                    | Event::Touch(touch::Event::FingerMoved { .. }) => {
                        if let Some(scrollbar) = scrollbars.y {
                            let Some(cursor_position) =
                                cursor.land().position()
                            else {
                                return;
                            };

                            state.scroll_y_to(
                                scrollbar.scroll_percentage_y(
                                    scroller_grabbed_at,
                                    cursor_position,
                                ),
                                bounds,
                                content_bounds,
                            );

                            let _ = notify_scroll(
                                state,
                                &self.on_scroll,
                                bounds,
                                content_bounds,
                                shell,
                            );

                            shell.capture_event();
                        }
                    }
                    _ => {}
                }
            } else if mouse_over_y_scrollbar {
                match event {
                    Event::Mouse(mouse::Event::ButtonPressed(
                        mouse::Button::Left,
                    ))
                    | Event::Touch(touch::Event::FingerPressed { .. }) => {
                        let Some(cursor_position) = cursor.position() else {
                            return;
                        };

                        if let (Some(scroller_grabbed_at), Some(scrollbar)) = (
                            scrollbars.grab_y_scroller(cursor_position),
                            scrollbars.y,
                        ) {
                            state.scroll_y_to(
                                scrollbar.scroll_percentage_y(
                                    scroller_grabbed_at,
                                    cursor_position,
                                ),
                                bounds,
                                content_bounds,
                            );

                            state.interaction = Interaction::YScrollerGrabbed(
                                scroller_grabbed_at,
                            );

                            let _ = notify_scroll(
                                state,
                                &self.on_scroll,
                                bounds,
                                content_bounds,
                                shell,
                            );
                        }

                        shell.capture_event();
                    }
                    _ => {}
                }
            }

            if let Some(scroller_grabbed_at) = state.x_scroller_grabbed_at() {
                match event {
                    Event::Mouse(mouse::Event::CursorMoved { .. })
                    | Event::Touch(touch::Event::FingerMoved { .. }) => {
                        let Some(cursor_position) = cursor.land().position()
                        else {
                            return;
                        };

                        if let Some(scrollbar) = scrollbars.x {
                            state.scroll_x_to(
                                scrollbar.scroll_percentage_x(
                                    scroller_grabbed_at,
                                    cursor_position,
                                ),
                                bounds,
                                content_bounds,
                            );

                            let _ = notify_scroll(
                                state,
                                &self.on_scroll,
                                bounds,
                                content_bounds,
                                shell,
                            );
                        }

                        shell.capture_event();
                    }
                    _ => {}
                }
            } else if mouse_over_x_scrollbar {
                match event {
                    Event::Mouse(mouse::Event::ButtonPressed(
                        mouse::Button::Left,
                    ))
                    | Event::Touch(touch::Event::FingerPressed { .. }) => {
                        let Some(cursor_position) = cursor.position() else {
                            return;
                        };

                        if let (Some(scroller_grabbed_at), Some(scrollbar)) = (
                            scrollbars.grab_x_scroller(cursor_position),
                            scrollbars.x,
                        ) {
                            state.scroll_x_to(
                                scrollbar.scroll_percentage_x(
                                    scroller_grabbed_at,
                                    cursor_position,
                                ),
                                bounds,
                                content_bounds,
                            );

                            state.interaction = Interaction::XScrollerGrabbed(
                                scroller_grabbed_at,
                            );

                            let _ = notify_scroll(
                                state,
                                &self.on_scroll,
                                bounds,
                                content_bounds,
                                shell,
                            );

                            shell.capture_event();
                        }
                    }
                    _ => {}
                }
            }

            if matches!(state.interaction, Interaction::AutoScrolling { .. })
                && matches!(
                    event,
                    Event::Mouse(
                        mouse::Event::ButtonPressed(_)
                            | mouse::Event::WheelScrolled { .. }
                    ) | Event::Touch(_)
                        | Event::Keyboard(_)
                )
            {
                state.interaction = Interaction::None;
                shell.capture_event();
                shell.invalidate_layout();
                shell.request_redraw();
                return;
            }

            if state.last_scrolled.is_none()
                || !matches!(
                    event,
                    Event::Mouse(mouse::Event::WheelScrolled { .. })
                )
            {
                let translation =
                    state.translation(self.direction, bounds, content_bounds);

                let cursor = match cursor_over_scrollable {
                    Some(cursor_position)
                        if !(mouse_over_x_scrollbar
                            || mouse_over_y_scrollbar) =>
                    {
                        mouse::Cursor::Available(cursor_position + translation)
                    }
                    _ => cursor.levitate() + translation,
                };

                let had_input_method = shell.input_method().is_enabled();

                let mut c_event = match event.clone() {
                    Event::Dnd(dnd::DndEvent::Offer(
                        id,
                        dnd::OfferEvent::Enter {
                            x,
                            y,
                            mime_types,
                            surface,
                        },
                    )) => Event::Dnd(dnd::DndEvent::Offer(
                        id.clone(),
                        dnd::OfferEvent::Enter {
                            x: x + translation.x as f64,
                            y: y + translation.y as f64,
                            mime_types: mime_types.clone(),
                            surface: surface.clone(),
                        },
                    )),
                    Event::Dnd(dnd::DndEvent::Offer(
                        id,
                        dnd::OfferEvent::Motion { x, y },
                    )) => Event::Dnd(dnd::DndEvent::Offer(
                        id.clone(),
                        dnd::OfferEvent::Motion {
                            x: x + translation.x as f64,
                            y: y + translation.y as f64,
                        },
                    )),
                    Event::Touch(touch::Event::FingerLifted {
                        id,
                        position,
                    }) if matches!(
                        state.interaction,
                        Interaction::TouchScrolling(_)
                    ) =>
                    {
                        Event::Touch(touch::Event::FingerLost { id, position })
                    }
                    e => e,
                };
                self.content.as_widget_mut().update(
                    &mut tree.children[0],
                    &c_event,
                    content.with_virtual_offset(
                        translation + layout.virtual_offset(),
                    ),
                    cursor,
                    renderer,
                    clipboard,
                    shell,
                    &Rectangle {
                        y: bounds.y + translation.y,
                        x: bounds.x + translation.x,
                        ..bounds
                    },
                );

                if !had_input_method
                    && let InputMethod::Enabled { cursor, .. } =
                        shell.input_method_mut()
                {
                    *cursor = *cursor - translation;
                }
            };

            if matches!(
                event,
                Event::Mouse(mouse::Event::CursorMoved { .. })
                    | Event::Touch(touch::Event::FingerPressed { .. })
            ) {
                state.suppress_touch_hover = false;
            }

            if matches!(
                event,
                Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left))
                    | Event::Touch(
                        touch::Event::FingerLifted { .. }
                            | touch::Event::FingerLost { .. }
                    )
            ) {
                if matches!(state.interaction, Interaction::TouchScrolling(_)) {
                    state.suppress_touch_hover = true;
                }
                state.interaction = Interaction::None;
                state.touch_press_start = None;
                return;
            }

            if let Event::Touch(touch::Event::FingerPressed { .. }) = event {
                state.touch_press_start = cursor_over_scrollable;
            }

            if shell.is_event_captured() {
                return;
            }

            match event {
                Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                    if cursor_over_scrollable.is_none() {
                        return;
                    }

                    let now = Instant::now();

                    let (delta, precise) = match *delta {
                        mouse::ScrollDelta::Lines { x, y } => {
                            let is_shift_pressed =
                                state.keyboard_modifiers.shift();

                            // macOS automatically inverts the axes when Shift is pressed
                            let (x, y) = if cfg!(target_os = "macos")
                                && is_shift_pressed
                            {
                                (y, x)
                            } else {
                                (x, y)
                            };

                            let movement = if !is_shift_pressed {
                                Vector::new(x, y)
                            } else {
                                Vector::new(y, x)
                            };

                            // Use `page_size^(2/3)` as the distance of a
                            // wheel detent.
                            let step = Vector::new(
                                wheel_step(bounds.width),
                                wheel_step(bounds.height),
                            );

                            (
                                -Vector::new(
                                    movement.x * step.x,
                                    movement.y * step.y,
                                ),
                                false,
                            )
                        }
                        mouse::ScrollDelta::Pixels { x, y } => {
                            (-Vector::new(x, y), true)
                        }
                    };

                    let stopping = precise && delta.x == 0.0 && delta.y == 0.0;
                    let delta = self.direction.align(delta);

                    let moved = if stopping {
                        state.end_precise_scroll(now, bounds, content_bounds)
                    } else if precise {
                        state.scroll_precise(delta, bounds, content_bounds, now)
                    } else {
                        state.scroll_wheel(delta, bounds, content_bounds, now)
                    };

                    if moved || state.smooth_scroll.is_some() {
                        shell.request_redraw();
                    }

                    let has_scrolled = notify_scroll(
                        state,
                        &self.on_scroll,
                        bounds,
                        content_bounds,
                        shell,
                    );

                    let in_transaction = state.last_scrolled.is_some();

                    if has_scrolled || moved || in_transaction {
                        shell.capture_event();
                    }
                }
                Event::Mouse(mouse::Event::ButtonPressed(
                    mouse::Button::Middle,
                )) if self.auto_scroll
                    && matches!(state.interaction, Interaction::None) =>
                {
                    let Some(origin) = cursor_over_scrollable else {
                        return;
                    };

                    state.cancel_smooth_scroll();

                    state.interaction = Interaction::AutoScrolling {
                        origin,
                        current: origin,
                        last_frame: None,
                    };

                    shell.capture_event();
                    shell.invalidate_layout();
                    shell.request_redraw();
                }
                Event::Touch(event)
                    if matches!(
                        state.interaction,
                        Interaction::TouchScrolling(_)
                    ) || (!mouse_over_y_scrollbar
                        && !mouse_over_x_scrollbar) =>
                {
                    match event {
                        touch::Event::FingerPressed { .. } => {
                            let Some(position) = cursor_over_scrollable else {
                                return;
                            };

                            state.cancel_smooth_scroll();

                            state.interaction =
                                Interaction::TouchScrolling(position);
                        }
                        touch::Event::FingerMoved { .. } => {
                            let Some(cursor_position) = cursor.position()
                            else {
                                return;
                            };

                            if !matches!(
                                state.interaction,
                                Interaction::TouchScrolling(_)
                            ) {
                                let Some(start) = state.touch_press_start
                                else {
                                    return;
                                };
                                if start.distance(cursor_position)
                                    < crate::DRAG_DEADBAND_DISTANCE
                                {
                                    return;
                                }
                                state.interaction =
                                    Interaction::TouchScrolling(start);
                            }

                            let Interaction::TouchScrolling(
                                scroll_box_touched_at,
                            ) = state.interaction
                            else {
                                return;
                            };

                            let delta = Vector::new(
                                scroll_box_touched_at.x - cursor_position.x,
                                scroll_box_touched_at.y - cursor_position.y,
                            );

                            state.scroll(
                                self.direction.align(delta),
                                bounds,
                                content_bounds,
                            );

                            state.interaction =
                                Interaction::TouchScrolling(cursor_position);

                            // TODO: bubble up touch movements if not consumed.
                            let _ = notify_scroll(
                                state,
                                &self.on_scroll,
                                bounds,
                                content_bounds,
                                shell,
                            );
                        }
                        _ => {}
                    }

                    shell.capture_event();
                }
                Event::Mouse(mouse::Event::CursorMoved { position }) => {
                    if let Interaction::AutoScrolling {
                        origin,
                        last_frame,
                        ..
                    } = state.interaction
                    {
                        let delta = *position - origin;

                        state.interaction = Interaction::AutoScrolling {
                            origin,
                            current: *position,
                            last_frame,
                        };

                        if (delta.x.abs() >= AUTOSCROLL_DEADZONE
                            || delta.y.abs() >= AUTOSCROLL_DEADZONE)
                            && last_frame.is_none()
                        {
                            shell.request_redraw();
                        }
                    }
                }
                Event::Keyboard(keyboard::Event::ModifiersChanged(
                    modifiers,
                )) => {
                    state.keyboard_modifiers = *modifiers;
                }
                Event::Window(window::Event::RedrawRequested(now)) => {
                    if let Interaction::AutoScrolling {
                        origin,
                        current,
                        last_frame,
                    } = state.interaction
                    {
                        if last_frame == Some(*now) {
                            shell.request_redraw();
                            return;
                        }

                        state.interaction = Interaction::AutoScrolling {
                            origin,
                            current,
                            last_frame: None,
                        };

                        let mut delta = current - origin;

                        if delta.x.abs() < AUTOSCROLL_DEADZONE {
                            delta.x = 0.0;
                        }

                        if delta.y.abs() < AUTOSCROLL_DEADZONE {
                            delta.y = 0.0;
                        }
                        if delta.x != 0.0 || delta.y != 0.0 {
                            let time_delta =
                                if let Some(last_frame) = last_frame {
                                    *now - last_frame
                                } else {
                                    Duration::ZERO
                                };

                            let scroll_factor = time_delta.as_secs_f32();
                            state.scroll(
                                self.direction.align(Vector::new(
                                    delta.x.signum()
                                        * delta
                                            .x
                                            .abs()
                                            .powf(AUTOSCROLL_SMOOTHNESS)
                                        * scroll_factor,
                                    delta.y.signum()
                                        * delta
                                            .y
                                            .abs()
                                            .powf(AUTOSCROLL_SMOOTHNESS)
                                        * scroll_factor,
                                )),
                                bounds,
                                content_bounds,
                            );

                            let has_scrolled = notify_scroll(
                                state,
                                &self.on_scroll,
                                bounds,
                                content_bounds,
                                shell,
                            );

                            if has_scrolled || time_delta.is_zero() {
                                state.interaction =
                                    Interaction::AutoScrolling {
                                        origin,
                                        current,
                                        last_frame: Some(*now),
                                    };

                                shell.request_redraw();
                            }

                            return;
                        }
                    }

                    let animating =
                        state.smooth_tick(*now, bounds, content_bounds);

                    let _ = notify_viewport(
                        state,
                        &self.on_scroll,
                        bounds,
                        content_bounds,
                        shell,
                    );

                    if animating {
                        shell.request_redraw();
                    }
                }
                _ => {}
            }
        };

        update();

        let status = if state.scrollers_grabbed() {
            Status::Dragged {
                is_horizontal_scrollbar_dragged: state
                    .x_scroller_grabbed_at()
                    .is_some(),
                is_vertical_scrollbar_dragged: state
                    .y_scroller_grabbed_at()
                    .is_some(),
                is_horizontal_scrollbar_disabled: scrollbars.is_x_disabled(),
                is_vertical_scrollbar_disabled: scrollbars.is_y_disabled(),
            }
        } else if cursor_over_scrollable.is_some() {
            Status::Hovered {
                is_horizontal_scrollbar_hovered: mouse_over_x_scrollbar,
                is_vertical_scrollbar_hovered: mouse_over_y_scrollbar,
                is_horizontal_scrollbar_disabled: scrollbars.is_x_disabled(),
                is_vertical_scrollbar_disabled: scrollbars.is_y_disabled(),
            }
        } else {
            Status::Active {
                is_horizontal_scrollbar_disabled: scrollbars.is_x_disabled(),
                is_vertical_scrollbar_disabled: scrollbars.is_y_disabled(),
            }
        };

        if let Event::Window(window::Event::RedrawRequested(_now)) = event {
            self.last_status = Some(status);
        }

        if last_offsets != (state.offset_x, state.offset_y)
            || self
                .last_status
                .is_some_and(|last_status| last_status != status)
        {
            shell.request_redraw();
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        defaults: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();

        let bounds = layout.bounds();
        let content_layout = layout.children().next().unwrap();
        let content_bounds = content_layout.bounds();

        let Some(visible_bounds) = bounds.intersection(viewport) else {
            return;
        };

        let scrollbars =
            Scrollbars::new(state, self.direction, bounds, content_bounds);

        let cursor_over_scrollable = cursor.position_over(bounds);
        let (mouse_over_y_scrollbar, mouse_over_x_scrollbar) =
            scrollbars.is_mouse_over(cursor);

        let translation =
            state.translation(self.direction, bounds, content_bounds);

        // Snap the translation to the physical pixel grid to keep content
        // crisp while still allowing sub-pixel animation.
        let scale_factor = defaults.scale_factor as f32;
        let translation = if scale_factor > 0.0 {
            let scaled = translation * scale_factor;

            Vector::new(
                scaled.x.round() / scale_factor,
                scaled.y.round() / scale_factor,
            )
        } else {
            translation
        };

        let cursor = match cursor_over_scrollable {
            _ if state.suppress_touch_hover
                || matches!(
                    state.interaction,
                    Interaction::TouchScrolling(_)
                ) =>
            {
                mouse::Cursor::Unavailable
            }
            Some(cursor_position)
                if !(mouse_over_x_scrollbar || mouse_over_y_scrollbar) =>
            {
                mouse::Cursor::Available(cursor_position + translation)
            }
            _ => mouse::Cursor::Unavailable,
        };

        let style = theme.style(
            &self.class,
            self.last_status.unwrap_or(Status::Active {
                is_horizontal_scrollbar_disabled: false,
                is_vertical_scrollbar_disabled: false,
            }),
        );

        container::draw_background(renderer, &style.container, layout.bounds());

        // Draw inner content
        if scrollbars.active() {
            renderer.with_layer(visible_bounds, |renderer| {
                renderer.with_translation(
                    Vector::new(-translation.x, -translation.y),
                    |renderer| {
                        self.content.as_widget().draw(
                            &tree.children[0],
                            renderer,
                            theme,
                            defaults,
                            content_layout.with_virtual_offset(
                                translation + layout.virtual_offset(),
                            ),
                            cursor,
                            &Rectangle {
                                y: visible_bounds.y + translation.y,
                                x: visible_bounds.x + translation.x,
                                ..visible_bounds
                            },
                        );
                    },
                );
            });

            let draw_scrollbar =
                |renderer: &mut Renderer,
                 style: Rail,
                 scrollbar: &internals::Scrollbar| {
                    if scrollbar.bounds.width > 0.0
                        && scrollbar.bounds.height > 0.0
                        && (style.background.is_some()
                            || (style.border.color != Color::TRANSPARENT
                                && style.border.width > 0.0))
                    {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: scrollbar.bounds,
                                border: style.border,
                                ..renderer::Quad::default()
                            },
                            style.background.unwrap_or(Background::Color(
                                Color::TRANSPARENT,
                            )),
                        );
                    }

                    if let Some(scroller) = scrollbar.scroller
                        && scroller.bounds.width > 0.0
                        && scroller.bounds.height > 0.0
                        && (style.scroller.background
                            != Background::Color(Color::TRANSPARENT)
                            || (style.scroller.border.color
                                != Color::TRANSPARENT
                                && style.scroller.border.width > 0.0))
                    {
                        renderer.fill_quad(
                            renderer::Quad {
                                bounds: scroller.bounds,
                                border: style.scroller.border,
                                ..renderer::Quad::default()
                            },
                            style.scroller.background,
                        );
                    }
                };

            renderer.with_layer(
                Rectangle {
                    width: (visible_bounds.width + 2.0).min(viewport.width),
                    height: (visible_bounds.height + 2.0).min(viewport.height),
                    ..visible_bounds
                },
                |renderer| {
                    if let Some(scrollbar) = scrollbars.y {
                        draw_scrollbar(
                            renderer,
                            style.vertical_rail,
                            &scrollbar,
                        );
                    }

                    if let Some(scrollbar) = scrollbars.x {
                        draw_scrollbar(
                            renderer,
                            style.horizontal_rail,
                            &scrollbar,
                        );
                    }

                    if let (Some(x), Some(y)) = (scrollbars.x, scrollbars.y) {
                        let background =
                            style.gap.or(style.container.background);

                        if let Some(background) = background {
                            renderer.fill_quad(
                                renderer::Quad {
                                    bounds: Rectangle {
                                        x: y.bounds.x,
                                        y: x.bounds.y,
                                        width: y.bounds.width,
                                        height: x.bounds.height,
                                    },
                                    ..renderer::Quad::default()
                                },
                                background,
                            );
                        }
                    }
                },
            );
        } else {
            self.content.as_widget().draw(
                &tree.children[0],
                renderer,
                theme,
                defaults,
                content_layout.with_virtual_offset(layout.virtual_offset()),
                cursor,
                &Rectangle {
                    x: visible_bounds.x + translation.x,
                    y: visible_bounds.y + translation.y,
                    ..visible_bounds
                },
            );
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let cursor_over_scrollable = cursor.position_over(bounds);

        let content_layout = layout.children().next().unwrap();
        let content_bounds = content_layout.bounds();

        let scrollbars =
            Scrollbars::new(state, self.direction, bounds, content_bounds);

        let (mouse_over_y_scrollbar, mouse_over_x_scrollbar) =
            scrollbars.is_mouse_over(cursor);

        if state.scrollers_grabbed() {
            return mouse::Interaction::None;
        }

        let translation =
            state.translation(self.direction, bounds, content_bounds);

        let cursor = match cursor_over_scrollable {
            _ if state.suppress_touch_hover
                || matches!(
                    state.interaction,
                    Interaction::TouchScrolling(_)
                ) =>
            {
                cursor.levitate() + translation
            }
            Some(cursor_position)
                if !(mouse_over_x_scrollbar || mouse_over_y_scrollbar) =>
            {
                mouse::Cursor::Available(cursor_position + translation)
            }
            _ => cursor.levitate() + translation,
        };

        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            content_layout.with_virtual_offset(layout.virtual_offset()),
            cursor,
            &Rectangle {
                y: bounds.y + translation.y,
                x: bounds.x + translation.x,
                ..bounds
            },
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();
        let content_layout = layout.children().next().unwrap();
        let content_bounds = content_layout.bounds();
        let visible_bounds = bounds.intersection(viewport).unwrap_or(*viewport);
        let offset = state.translation(self.direction, bounds, content_bounds);

        let overlay = self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout
                .children()
                .next()
                .unwrap()
                .with_virtual_offset(translation + layout.virtual_offset()),
            renderer,
            &visible_bounds,
            translation - offset,
        );

        let icon = if let Interaction::AutoScrolling { origin, .. } =
            state.interaction
        {
            let scrollbars =
                Scrollbars::new(state, self.direction, bounds, content_bounds);

            Some(overlay::Element::new(Box::new(AutoScrollIcon {
                origin,
                vertical: scrollbars.y.is_some(),
                horizontal: scrollbars.x.is_some(),
                class: &self.class,
            })))
        } else {
            None
        };

        match (overlay, icon) {
            (None, None) => None,
            (None, Some(icon)) => Some(icon),
            (Some(overlay), None) => Some(overlay),
            (Some(overlay), Some(icon)) => Some(overlay::Element::new(
                Box::new(overlay::Group::with_children(vec![overlay, icon])),
            )),
        }
    }

    #[cfg(feature = "a11y")]
    fn a11y_nodes(
        &self,
        layout: Layout<'_>,
        state: &Tree,
        cursor: mouse::Cursor,
    ) -> iced_accessibility::A11yTree {
        use iced_accessibility::{
            A11yId, A11yNode, A11yTree,
            accesskit::{Node, NodeId, Rect, Role},
        };
        if !matches!(state.state, tree::State::Some(_)) {
            return A11yTree::default();
        }
        let window = layout.bounds();
        let is_hovered = cursor.is_over(window);
        let Rectangle {
            x,
            y,
            width,
            height,
        } = window;

        let my_state = state.state.downcast_ref::<State>();
        let content = layout.children().next().unwrap();
        let content_bounds = content.bounds();

        let translation = my_state.translation(
            self.direction,
            layout.bounds(),
            content_bounds,
        );

        let child_layout = layout.children().next().unwrap();
        let child_tree = &state.children[0];
        let child_tree = self.content.as_widget().a11y_nodes(
            child_layout
                .with_virtual_offset(translation + layout.virtual_offset()),
            &child_tree,
            cursor,
        );
        let bounds = Rect::new(
            x as f64,
            y as f64,
            (x + width) as f64,
            (y + height) as f64,
        );
        let mut node = Node::new(Role::ScrollView);
        node.set_bounds(bounds);
        if let Some(name) = self.name.as_ref() {
            node.set_label(name.clone());
        }
        match self.description.as_ref() {
            Some(iced_accessibility::Description::Id(id)) => {
                node.set_described_by(
                    id.iter()
                        .cloned()
                        .map(|id| NodeId::from(id))
                        .collect::<Vec<_>>(),
                );
            }
            Some(iced_accessibility::Description::Text(text)) => {
                node.set_description(text.clone());
            }
            None => {}
        }

        // TODO hover
        // if is_hovered {
        //     node.set_hovered();
        // }

        if let Some(label) = self.label.as_ref() {
            node.set_labelled_by(label.clone());
        }

        let content = layout.children().next().unwrap();
        let content_bounds = content.bounds();

        let mut scrollbar_node = Node::new(Role::ScrollBar);
        if matches!(state.state, tree::State::Some(_)) {
            let state = state.state.downcast_ref::<State>();
            let scrollbars = Scrollbars::new(
                state,
                self.direction,
                content_bounds,
                content_bounds,
            );
            for (window, content, offset, scrollbar) in scrollbars
                .x
                .iter()
                .map(|s| {
                    (window.width, content_bounds.width, state.offset_x, s)
                })
                .chain(scrollbars.y.iter().map(|s| {
                    (window.height, content_bounds.height, state.offset_y, s)
                }))
            {
                let scrollbar_bounds = scrollbar.total_bounds;
                let is_hovered = cursor.is_over(scrollbar_bounds);
                let Rectangle {
                    x,
                    y,
                    width,
                    height,
                } = scrollbar_bounds;
                let bounds = Rect::new(
                    x as f64,
                    y as f64,
                    (x + width) as f64,
                    (y + height) as f64,
                );
                scrollbar_node.set_bounds(bounds);
                // TODO: hover
                // if is_hovered {
                //     scrollbar_node.set_hovered();
                // }
                scrollbar_node
                    .set_controls(vec![A11yId::Widget(self.id.clone()).into()]);
                scrollbar_node.set_numeric_value(
                    100.0 * offset.absolute(window, content) as f64
                        / scrollbar_bounds.height as f64,
                );
            }
        }

        let child_tree = A11yTree::join(
            [
                child_tree,
                A11yTree::leaf(scrollbar_node, self.scrollbar_id.clone()),
            ]
            .into_iter(),
        );
        A11yTree::node_with_child_tree(
            A11yNode::new(node, self.id.clone()),
            child_tree,
        )
    }

    fn id(&self) -> Option<Id> {
        Some(Id(Internal::Set(vec![
            self.id.0.clone(),
            self.scrollbar_id.0.clone(),
        ])))
    }

    fn set_id(&mut self, id: Id) {
        if let Id(Internal::Set(list)) = id {
            if list.len() == 2 {
                self.id.0 = list[0].clone();
                self.scrollbar_id.0 = list[1].clone();
            }
        }
    }

    fn drag_destinations(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        dnd_rectangles: &mut crate::core::clipboard::DndDestinationRectangles,
    ) {
        let my_state = tree.state.downcast_ref::<State>();
        if let Some((c_layout, c_state)) =
            layout.children().zip(tree.children.iter()).next()
        {
            let mut my_dnd_rectangles = DndDestinationRectangles::new();
            let translation = my_state.translation(
                self.direction,
                layout.bounds(),
                c_layout.bounds(),
            );
            self.content.as_widget().drag_destinations(
                c_state,
                c_layout
                    .with_virtual_offset(translation + layout.virtual_offset()),
                renderer,
                &mut my_dnd_rectangles,
            );
            let mut my_dnd_rectangles = my_dnd_rectangles.into_rectangles();

            let bounds = layout.bounds();
            let content_bounds = c_layout.bounds();
            for r in &mut my_dnd_rectangles {
                let translation = my_state.translation(
                    self.direction,
                    bounds,
                    content_bounds,
                );
                r.rectangle.x -= translation.x as f64;
                r.rectangle.y -= translation.y as f64;
            }
            dnd_rectangles.append(&mut my_dnd_rectangles);
        }
    }
}

struct AutoScrollIcon<'a, Class> {
    origin: Point,
    vertical: bool,
    horizontal: bool,
    class: &'a Class,
}

impl<Class> AutoScrollIcon<'_, Class> {
    const SIZE: f32 = 40.0;
    const DOT: f32 = Self::SIZE / 10.0;
    const PADDING: f32 = Self::SIZE / 10.0;
}

impl<Message, Theme, Renderer> core::Overlay<Message, Theme, Renderer>
    for AutoScrollIcon<'_, Theme::Class<'_>>
where
    Renderer: text::Renderer,
    Theme: Catalog,
{
    fn layout(&mut self, _renderer: &Renderer, _bounds: Size) -> layout::Node {
        layout::Node::new(Size::new(Self::SIZE, Self::SIZE))
            .move_to(self.origin - Vector::new(Self::SIZE, Self::SIZE) / 2.0)
    }

    fn draw(
        &self,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
    ) {
        let bounds = layout.bounds();
        let style = theme
            .style(
                self.class,
                Status::Active {
                    is_horizontal_scrollbar_disabled: false,
                    is_vertical_scrollbar_disabled: false,
                },
            )
            .auto_scroll;

        renderer.with_layer(Rectangle::INFINITE, |renderer| {
            renderer.fill_quad(
                renderer::Quad {
                    bounds,
                    border: style.border,
                    shadow: style.shadow,
                    snap: false,
                },
                style.background,
            );

            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle::new(
                        bounds.center()
                            - Vector::new(Self::DOT, Self::DOT) / 2.0,
                        Size::new(Self::DOT, Self::DOT),
                    ),
                    border: border::rounded(bounds.width),
                    snap: false,
                    ..renderer::Quad::default()
                },
                style.icon,
            );

            let arrow = core::Text {
                content: String::new(),
                bounds: bounds.size(),
                size: Pixels::from(12),
                line_height: text::LineHeight::Relative(1.0),
                font: Renderer::ICON_FONT,
                align_x: text::Alignment::Center,
                align_y: alignment::Vertical::Center,
                shaping: text::Shaping::Basic,
                wrapping: text::Wrapping::None,
                ellipsize: text::Ellipsize::None,
            };

            if self.vertical {
                renderer.fill_text(
                    core::Text {
                        content: Renderer::SCROLL_UP_ICON.to_string(),
                        align_y: alignment::Vertical::Top,
                        ..arrow
                    },
                    Point::new(bounds.center_x(), bounds.y + Self::PADDING),
                    style.icon,
                    bounds,
                );

                renderer.fill_text(
                    core::Text {
                        content: Renderer::SCROLL_DOWN_ICON.to_string(),
                        align_y: alignment::Vertical::Bottom,
                        ..arrow
                    },
                    Point::new(
                        bounds.center_x(),
                        bounds.y + bounds.height - Self::PADDING - 0.5,
                    ),
                    style.icon,
                    bounds,
                );
            }

            if self.horizontal {
                renderer.fill_text(
                    core::Text {
                        content: Renderer::SCROLL_LEFT_ICON.to_string(),
                        align_x: text::Alignment::Left,
                        ..arrow
                    },
                    Point::new(
                        bounds.x + Self::PADDING + 1.0,
                        bounds.center_y() + 1.0,
                    ),
                    style.icon,
                    bounds,
                );

                renderer.fill_text(
                    core::Text {
                        content: Renderer::SCROLL_RIGHT_ICON.to_string(),
                        align_x: text::Alignment::Right,
                        ..arrow
                    },
                    Point::new(
                        bounds.x + bounds.width - Self::PADDING - 1.0,
                        bounds.center_y() + 1.0,
                    ),
                    style.icon,
                    bounds,
                );
            }
        });
    }

    fn index(&self) -> f32 {
        f32::MAX
    }
}

impl<'a, Message, Theme, Renderer>
    From<Scrollable<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a + Catalog,
    Renderer: 'a + text::Renderer,
{
    fn from(
        text_input: Scrollable<'a, Message, Theme, Renderer>,
    ) -> Element<'a, Message, Theme, Renderer> {
        Element::new(text_input)
    }
}

/// Produces a [`Task`] that snaps the [`Scrollable`] with the given [`Id`]
/// to the provided [`RelativeOffset`].
pub fn snap_to<T>(id: Id, offset: RelativeOffset<Option<f32>>) -> Task<T> {
    task::effect(Action::widget(operation::scrollable::snap_to(id, offset)))
}

/// Produces a [`Task`] that scrolls the [`Scrollable`] with the given [`Id`]
/// to the provided [`AbsoluteOffset`].
pub fn scroll_to<T>(id: Id, offset: AbsoluteOffset<Option<f32>>) -> Task<T> {
    task::effect(Action::widget(operation::scrollable::scroll_to(id, offset)))
}

/// Produces a [`Task`] that scrolls the [`Scrollable`] with the given [`Id`]
/// by the provided [`AbsoluteOffset`].
pub fn scroll_by<T>(id: Id, offset: AbsoluteOffset) -> Task<T> {
    task::effect(Action::widget(operation::scrollable::scroll_by(id, offset)))
}

fn notify_scroll<Message>(
    state: &mut State,
    on_scroll: &Option<Box<dyn Fn(Viewport) -> Message + '_>>,
    bounds: Rectangle,
    content_bounds: Rectangle,
    shell: &mut Shell<'_, Message>,
) -> bool {
    if notify_viewport(state, on_scroll, bounds, content_bounds, shell) {
        state.last_scrolled = Some(Instant::now());

        true
    } else {
        false
    }
}

fn notify_viewport<Message>(
    state: &mut State,
    on_scroll: &Option<Box<dyn Fn(Viewport) -> Message + '_>>,
    bounds: Rectangle,
    content_bounds: Rectangle,
    shell: &mut Shell<'_, Message>,
) -> bool {
    if content_bounds.width <= bounds.width
        && content_bounds.height <= bounds.height
    {
        return false;
    }

    let viewport = Viewport {
        offset_x: state.offset_x,
        offset_y: state.offset_y,
        bounds,
        content_bounds,
    };

    // Don't publish redundant viewports to shell
    if let Some(last_notified) = state.last_notified {
        let last_relative_offset = last_notified.relative_offset();
        let current_relative_offset = viewport.relative_offset();

        let last_absolute_offset = last_notified.absolute_offset();
        let current_absolute_offset = viewport.absolute_offset();

        let unchanged = |a: f32, b: f32| {
            (a - b).abs() <= f32::EPSILON || (a.is_nan() && b.is_nan())
        };

        if last_notified.bounds == bounds
            && last_notified.content_bounds == content_bounds
            && unchanged(last_relative_offset.x, current_relative_offset.x)
            && unchanged(last_relative_offset.y, current_relative_offset.y)
            && unchanged(last_absolute_offset.x, current_absolute_offset.x)
            && unchanged(last_absolute_offset.y, current_absolute_offset.y)
        {
            return false;
        }
    }

    state.last_notified = Some(viewport);
    if let Some(on_scroll) = on_scroll {
        shell.publish(on_scroll(viewport));
    }

    true
}

/// Returns the maximum scroll offset for the given bounds.
fn scroll_max(bounds: Rectangle, content_bounds: Rectangle) -> Vector<f32> {
    Vector::new(
        (content_bounds.width - bounds.width).max(0.0),
        (content_bounds.height - bounds.height).max(0.0),
    )
}

#[derive(Debug, Clone, Copy)]
struct State {
    offset_y: Offset,
    offset_x: Offset,
    interaction: Interaction,
    keyboard_modifiers: keyboard::Modifiers,
    last_notified: Option<Viewport>,
    last_scrolled: Option<Instant>,
    is_scrollbar_visible: bool,
    touch_press_start: Option<Point>,
    suppress_touch_hover: bool,
    smooth_scroll: Option<SmoothScroll>,
}

/// The state of an in-progress smooth scroll animation, as well as the
/// velocity of the latest precise scroll gesture.
#[derive(Debug, Clone, Copy)]
struct SmoothScroll {
    /// The offset the content is animating towards, if any.
    target: Option<Vector<f32>>,
    /// The timestamp of the last animation frame.
    last_frame: Option<Instant>,
    /// The timestamp of the last scroll input.
    last_input: Instant,
    /// The velocity of the last precise scroll gesture, in logical pixels
    /// per second.
    velocity_x: f32,
    /// The velocity of the last precise scroll gesture, in logical pixels
    /// per second.
    velocity_y: f32,
    /// Whether the last input was precise (for example, a touchpad).
    precise: bool,
    /// Whether the content is currently gliding with momentum.
    gliding: bool,
}

#[derive(Debug, Clone, Copy)]
enum Interaction {
    None,
    YScrollerGrabbed(f32),
    XScrollerGrabbed(f32),
    TouchScrolling(Point),
    AutoScrolling {
        origin: Point,
        current: Point,
        last_frame: Option<Instant>,
    },
}

impl Default for State {
    fn default() -> Self {
        Self {
            offset_y: Offset::Absolute(0.0),
            offset_x: Offset::Absolute(0.0),
            interaction: Interaction::None,
            keyboard_modifiers: keyboard::Modifiers::default(),
            last_notified: None,
            last_scrolled: None,
            is_scrollbar_visible: true,
            touch_press_start: None,
            suppress_touch_hover: false,
            smooth_scroll: None,
        }
    }
}

impl operation::Scrollable for State {
    fn snap_to(&mut self, offset: RelativeOffset<Option<f32>>) {
        State::snap_to(self, offset);
    }

    fn scroll_to(&mut self, offset: AbsoluteOffset<Option<f32>>) {
        State::scroll_to(self, offset);
    }

    fn scroll_by(
        &mut self,
        offset: AbsoluteOffset,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) {
        State::scroll_by(self, offset, bounds, content_bounds);
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Offset {
    Absolute(f32),
    Relative(f32),
}

impl Offset {
    fn absolute(self, viewport: f32, content: f32) -> f32 {
        match self {
            Offset::Absolute(absolute) => {
                absolute.min((content - viewport).max(0.0))
            }
            Offset::Relative(percentage) => {
                ((content - viewport) * percentage).max(0.0)
            }
        }
    }

    fn translation(
        self,
        viewport: f32,
        content: f32,
        alignment: Anchor,
    ) -> f32 {
        let offset = self.absolute(viewport, content);

        match alignment {
            Anchor::Start => offset,
            Anchor::End => ((content - viewport).max(0.0) - offset).max(0.0),
        }
    }
}

/// The current [`Viewport`] of the [`Scrollable`].
#[derive(Debug, Clone, Copy)]
pub struct Viewport {
    offset_x: Offset,
    offset_y: Offset,
    bounds: Rectangle,
    content_bounds: Rectangle,
}

impl Viewport {
    /// Returns the [`AbsoluteOffset`] of the current [`Viewport`].
    pub fn absolute_offset(&self) -> AbsoluteOffset {
        let x = self
            .offset_x
            .absolute(self.bounds.width, self.content_bounds.width);
        let y = self
            .offset_y
            .absolute(self.bounds.height, self.content_bounds.height);

        AbsoluteOffset { x, y }
    }

    /// Returns the [`AbsoluteOffset`] of the current [`Viewport`], but with its
    /// alignment reversed.
    ///
    /// This method can be useful to switch the alignment of a [`Scrollable`]
    /// while maintaining its scrolling position.
    pub fn absolute_offset_reversed(&self) -> AbsoluteOffset {
        let AbsoluteOffset { x, y } = self.absolute_offset();

        AbsoluteOffset {
            x: (self.content_bounds.width - self.bounds.width).max(0.0) - x,
            y: (self.content_bounds.height - self.bounds.height).max(0.0) - y,
        }
    }

    /// Returns the [`RelativeOffset`] of the current [`Viewport`].
    pub fn relative_offset(&self) -> RelativeOffset {
        let AbsoluteOffset { x, y } = self.absolute_offset();

        let x = x / (self.content_bounds.width - self.bounds.width);
        let y = y / (self.content_bounds.height - self.bounds.height);

        RelativeOffset { x, y }
    }

    /// Returns the bounds of the current [`Viewport`].
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }

    /// Returns the content bounds of the current [`Viewport`].
    pub fn content_bounds(&self) -> Rectangle {
        self.content_bounds
    }
}

impl State {
    fn new() -> Self {
        State::default()
    }

    fn scroll(
        &mut self,
        delta: Vector<f32>,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) {
        if bounds.height < content_bounds.height {
            self.offset_y = Offset::Absolute(
                (self.offset_y.absolute(bounds.height, content_bounds.height)
                    + delta.y)
                    .clamp(0.0, content_bounds.height - bounds.height),
            );
        }

        if bounds.width < content_bounds.width {
            self.offset_x = Offset::Absolute(
                (self.offset_x.absolute(bounds.width, content_bounds.width)
                    + delta.x)
                    .clamp(0.0, content_bounds.width - bounds.width),
            );
        }
    }

    /// Cancels any in-progress smooth scroll animation.
    fn cancel_smooth_scroll(&mut self) {
        self.smooth_scroll = None;
    }

    /// Applies a discrete (wheel) scroll by moving an animation target.
    ///
    /// Returns whether the content can scroll in the requested direction.
    fn scroll_wheel(
        &mut self,
        delta: Vector<f32>,
        bounds: Rectangle,
        content_bounds: Rectangle,
        now: Instant,
    ) -> bool {
        let max = scroll_max(bounds, content_bounds);
        let current = Vector::new(
            self.offset_x.absolute(bounds.width, content_bounds.width),
            self.offset_y.absolute(bounds.height, content_bounds.height),
        );

        let base = self
            .smooth_scroll
            .and_then(|smooth| smooth.target)
            .unwrap_or(current);
        let target = Vector::new(
            (base.x + delta.x).clamp(0.0, max.x),
            (base.y + delta.y).clamp(0.0, max.y),
        );

        if target.x == base.x && target.y == base.y {
            return false;
        }

        self.unsnap(bounds, content_bounds);

        let smooth = self.smooth_scroll.get_or_insert_with(|| SmoothScroll {
            target: Some(current),
            last_frame: None,
            last_input: now,
            velocity_x: 0.0,
            velocity_y: 0.0,
            precise: false,
            gliding: false,
        });

        smooth.target = Some(target);
        smooth.velocity_x = 0.0;
        smooth.velocity_y = 0.0;
        smooth.precise = false;
        smooth.gliding = false;
        smooth.last_input = now;

        true
    }

    /// Applies a precise (touchpad) scroll delta directly to the offset and
    /// tracks its velocity so the content can glide once the gesture ends.
    ///
    /// Returns whether the content moved.
    fn scroll_precise(
        &mut self,
        delta: Vector<f32>,
        bounds: Rectangle,
        content_bounds: Rectangle,
        now: Instant,
    ) -> bool {
        self.unsnap(bounds, content_bounds);

        let delta = delta * PRECISE_SCROLL_SCALE;
        let max = scroll_max(bounds, content_bounds);
        let current = Vector::new(
            self.offset_x.absolute(bounds.width, content_bounds.width),
            self.offset_y.absolute(bounds.height, content_bounds.height),
        );
        let next = Vector::new(
            (current.x + delta.x).clamp(0.0, max.x),
            (current.y + delta.y).clamp(0.0, max.y),
        );

        if next.x == current.x && next.y == current.y {
            return false;
        }

        self.offset_x = Offset::Absolute(next.x);
        self.offset_y = Offset::Absolute(next.y);

        let smooth = self.smooth_scroll.get_or_insert_with(|| SmoothScroll {
            target: None,
            last_frame: None,
            last_input: now,
            velocity_x: 0.0,
            velocity_y: 0.0,
            precise: true,
            gliding: false,
        });

        let dt = (now - smooth.last_input).as_secs_f32();

        if smooth.precise && dt > 0.0 && dt < 0.1 {
            let velocity_x =
                (delta.x / dt).clamp(-MAX_SCROLL_VELOCITY, MAX_SCROLL_VELOCITY);
            let velocity_y =
                (delta.y / dt).clamp(-MAX_SCROLL_VELOCITY, MAX_SCROLL_VELOCITY);

            // Weigh recent samples more heavily.
            smooth.velocity_x = smooth.velocity_x * 0.4 + velocity_x * 0.6;
            smooth.velocity_y = smooth.velocity_y * 0.4 + velocity_y * 0.6;
        } else {
            // The first sample or a long pause; don't derive a velocity from
            // it.
            smooth.velocity_x = 0.0;
            smooth.velocity_y = 0.0;
        }

        smooth.target = None;
        smooth.gliding = false;
        smooth.last_frame = None;
        smooth.last_input = now;
        smooth.precise = true;

        true
    }

    /// Marks the end of a precise (touchpad) scroll gesture, starting a
    /// momentum glide when the gesture was fast enough.
    ///
    /// On Wayland, backends forward the `axis_stop` event as a precise scroll
    /// without a delta.
    ///
    /// Returns whether a momentum glide was started.
    fn end_precise_scroll(
        &mut self,
        now: Instant,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) -> bool {
        let Some(smooth) = self.smooth_scroll.as_mut() else {
            return false;
        };

        if !smooth.precise || smooth.gliding {
            return false;
        }

        let lag = now.duration_since(smooth.last_input).as_secs_f32();

        // If the fingers were already resting, do not start a glide from
        // stale velocity.
        if lag >= MOMENTUM_MAX_LAG.as_secs_f32() {
            smooth.velocity_x = 0.0;
            smooth.velocity_y = 0.0;
            return false;
        }

        let decay = (-lag / MOMENTUM_LAG_DECAY).exp();
        smooth.velocity_x *= decay;
        smooth.velocity_y *= decay;

        let velocity = smooth.velocity_x.abs().max(smooth.velocity_y.abs());

        if velocity >= MOMENTUM_MIN_VELOCITY {
            let current = Vector::new(
                self.offset_x.absolute(bounds.width, content_bounds.width),
                self.offset_y.absolute(bounds.height, content_bounds.height),
            );

            smooth.target = Some(current);
            smooth.gliding = true;
            smooth.last_frame = None;
            return true;
        }

        smooth.velocity_x = 0.0;
        smooth.velocity_y = 0.0;
        return false;
    }

    /// Advances the smooth scrolling animation by one frame.
    ///
    /// Returns whether another frame is needed.
    fn smooth_tick(
        &mut self,
        now: Instant,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) -> bool {
        let Some(mut smooth) = self.smooth_scroll.take() else {
            return false;
        };

        // The same redraw event can be processed more than once per frame
        // (for example, when publishing a message rebuilds the user
        // interface). Only advance the animation once per frame, or the
        // scroll would move in large, uneven steps.
        if smooth.last_frame == Some(now) {
            self.smooth_scroll = Some(smooth);
            return true;
        }

        let max = scroll_max(bounds, content_bounds);
        let current = Vector::new(
            self.offset_x.absolute(bounds.width, content_bounds.width),
            self.offset_y.absolute(bounds.height, content_bounds.height),
        );

        let dt = match smooth.last_frame {
            Some(last_frame) => (now - last_frame).as_secs_f32(),
            None => 1.0 / 60.0,
        };
        let dt = if dt <= 0.0 {
            1.0 / 60.0
        } else {
            dt.min(MAX_FRAME_DELTA)
        };
        smooth.last_frame = Some(now);

        let mut next = current;
        let mut animating = false;

        if let Some(mut target) = smooth.target {
            if smooth.gliding {
                target.x += smooth.velocity_x * dt;
                target.y += smooth.velocity_y * dt;

                let decay = (-dt / MOMENTUM_TIME_CONSTANT).exp();
                smooth.velocity_x *= decay;
                smooth.velocity_y *= decay;
            }

            target.x = target.x.clamp(0.0, max.x);
            target.y = target.y.clamp(0.0, max.y);

            // Stop the glide at the edges of the content.
            if target.x <= 0.0 || target.x >= max.x {
                smooth.velocity_x = 0.0;
            }
            if target.y <= 0.0 || target.y >= max.y {
                smooth.velocity_y = 0.0;
            }

            let alpha = 1.0 - (-dt / SMOOTHING_TIME_CONSTANT).exp();

            next = Vector::new(
                current.x + (target.x - current.x) * alpha,
                current.y + (target.y - current.y) * alpha,
            );

            let settled = (target.x - next.x).abs() <= SNAP_DISTANCE
                && (target.y - next.y).abs() <= SNAP_DISTANCE
                && smooth.velocity_x.abs() < MOMENTUM_MIN_VELOCITY
                && smooth.velocity_y.abs() < MOMENTUM_MIN_VELOCITY;

            if settled {
                next = target;
                smooth.target = None;
                smooth.gliding = false;
                smooth.velocity_x = 0.0;
                smooth.velocity_y = 0.0;
            } else {
                smooth.target = Some(target);
                animating = true;
            }
        }

        self.offset_x = Offset::Absolute(next.x);
        self.offset_y = Offset::Absolute(next.y);

        // Keep tracking velocity while the gesture may still be active, so a
        // gesture end arriving shortly after the last movement can start a
        // momentum glide. A glide is never started once the input has been
        // quiet, so stopping the fingers stops the content.
        let stale = now.duration_since(smooth.last_input) >= MOMENTUM_MAX_LAG;

        if animating || (smooth.precise && !stale) {
            self.smooth_scroll = Some(smooth);
        }

        animating
    }

    fn scroll_y_to(
        &mut self,
        percentage: f32,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) {
        self.cancel_smooth_scroll();
        self.offset_y = Offset::Relative(percentage.clamp(0.0, 1.0));
        self.unsnap(bounds, content_bounds);
    }

    fn scroll_x_to(
        &mut self,
        percentage: f32,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) {
        self.cancel_smooth_scroll();
        self.offset_x = Offset::Relative(percentage.clamp(0.0, 1.0));
        self.unsnap(bounds, content_bounds);
    }

    fn snap_to(&mut self, offset: RelativeOffset<Option<f32>>) {
        self.cancel_smooth_scroll();

        if let Some(x) = offset.x {
            self.offset_x = Offset::Relative(x.clamp(0.0, 1.0));
        }

        if let Some(y) = offset.y {
            self.offset_y = Offset::Relative(y.clamp(0.0, 1.0));
        }
    }

    fn scroll_to(&mut self, offset: AbsoluteOffset<Option<f32>>) {
        self.cancel_smooth_scroll();

        if let Some(x) = offset.x {
            self.offset_x = Offset::Absolute(x.max(0.0));
        }

        if let Some(y) = offset.y {
            self.offset_y = Offset::Absolute(y.max(0.0));
        }
    }

    /// Scroll by the provided [`AbsoluteOffset`].
    fn scroll_by(
        &mut self,
        offset: AbsoluteOffset,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) {
        self.cancel_smooth_scroll();
        self.scroll(Vector::new(offset.x, offset.y), bounds, content_bounds);
    }

    /// Unsnaps the current scroll position, if snapped, given the bounds of the
    /// [`Scrollable`] and its contents.
    fn unsnap(&mut self, bounds: Rectangle, content_bounds: Rectangle) {
        self.offset_x = Offset::Absolute(
            self.offset_x.absolute(bounds.width, content_bounds.width),
        );
        self.offset_y = Offset::Absolute(
            self.offset_y.absolute(bounds.height, content_bounds.height),
        );
    }

    /// Returns the scrolling translation of the [`State`], given a [`Direction`],
    /// the bounds of the [`Scrollable`] and its contents.
    fn translation(
        &self,
        direction: Direction,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) -> Vector {
        Vector::new(
            if let Some(horizontal) = direction.horizontal() {
                self.offset_x.translation(
                    bounds.width,
                    content_bounds.width,
                    horizontal.alignment,
                )
            } else {
                0.0
            },
            if let Some(vertical) = direction.vertical() {
                self.offset_y.translation(
                    bounds.height,
                    content_bounds.height,
                    vertical.alignment,
                )
            } else {
                0.0
            },
        )
    }

    fn scrollers_grabbed(&self) -> bool {
        matches!(
            self.interaction,
            Interaction::YScrollerGrabbed(_) | Interaction::XScrollerGrabbed(_),
        )
    }

    pub fn y_scroller_grabbed_at(&self) -> Option<f32> {
        let Interaction::YScrollerGrabbed(at) = self.interaction else {
            return None;
        };

        Some(at)
    }

    pub fn x_scroller_grabbed_at(&self) -> Option<f32> {
        let Interaction::XScrollerGrabbed(at) = self.interaction else {
            return None;
        };

        Some(at)
    }
}

#[derive(Debug)]
/// State of both [`Scrollbar`]s.
struct Scrollbars {
    y: Option<internals::Scrollbar>,
    x: Option<internals::Scrollbar>,
}

impl Scrollbars {
    /// Create y and/or x scrollbar(s) if content is overflowing the [`Scrollable`] bounds.
    fn new(
        state: &State,
        direction: Direction,
        bounds: Rectangle,
        content_bounds: Rectangle,
    ) -> Self {
        let translation = state.translation(direction, bounds, content_bounds);

        let show_scrollbar_x = direction
            .horizontal()
            .filter(|_scrollbar| content_bounds.width > bounds.width);

        let show_scrollbar_y = direction
            .vertical()
            .filter(|_scrollbar| content_bounds.height > bounds.height);

        let y_scrollbar = if let Some(vertical) = show_scrollbar_y {
            let Scrollbar {
                width,
                margin,
                scroller_width,
                padding,
                ..
            } = *vertical;

            // Adjust the height of the vertical scrollbar if the horizontal scrollbar
            // is present
            let x_scrollbar_height = show_scrollbar_x
                .map_or(0.0, |h| h.width.max(h.scroller_width) + h.margin);

            let total_scrollbar_width =
                width.max(scroller_width) + 2.0 * margin;

            // Total bounds of the scrollbar + margin + scroller width
            let total_scrollbar_bounds = Rectangle {
                x: bounds.x + bounds.width - total_scrollbar_width,
                y: bounds.y + padding,
                width: total_scrollbar_width,
                height: (bounds.height - x_scrollbar_height - 2.0 * padding)
                    .max(0.0),
            };

            // Bounds of just the scrollbar
            let scrollbar_bounds = Rectangle {
                x: bounds.x + bounds.width
                    - total_scrollbar_width / 2.0
                    - width / 2.0,
                y: bounds.y + padding,
                width,
                height: (bounds.height - x_scrollbar_height - 2.0 * padding)
                    .max(0.0),
            };

            let ratio = bounds.height / content_bounds.height;

            let scroller = if ratio >= 1.0 {
                None
            } else {
                // min height for easier grabbing with super tall content
                let scroller_height =
                    (scrollbar_bounds.height * ratio).max(2.0);
                let scroller_offset =
                    translation.y * ratio * scrollbar_bounds.height
                        / bounds.height;

                let scroller_bounds = Rectangle {
                    x: bounds.x + bounds.width
                        - total_scrollbar_width / 2.0
                        - scroller_width / 2.0,
                    y: (scrollbar_bounds.y + scroller_offset).max(0.0),
                    width: scroller_width,
                    height: scroller_height,
                };

                Some(internals::Scroller {
                    bounds: scroller_bounds,
                })
            };

            Some(internals::Scrollbar {
                total_bounds: total_scrollbar_bounds,
                bounds: scrollbar_bounds,
                scroller,
                alignment: vertical.alignment,
                disabled: content_bounds.height <= bounds.height,
            })
        } else {
            None
        };

        let x_scrollbar = if let Some(horizontal) = show_scrollbar_x {
            let Scrollbar {
                width,
                margin,
                scroller_width,
                padding,
                ..
            } = *horizontal;

            // Need to adjust the width of the horizontal scrollbar if the vertical scrollbar
            // is present
            let scrollbar_y_width = y_scrollbar
                .map_or(0.0, |scrollbar| scrollbar.total_bounds.width);

            let total_scrollbar_height =
                width.max(scroller_width) + 2.0 * margin;

            // Total bounds of the scrollbar + margin + scroller width
            let total_scrollbar_bounds = Rectangle {
                x: bounds.x + padding,
                y: bounds.y + bounds.height - total_scrollbar_height,
                width: (bounds.width - scrollbar_y_width - 2.0 * padding)
                    .max(0.0),
                height: total_scrollbar_height,
            };

            // Bounds of just the scrollbar
            let scrollbar_bounds = Rectangle {
                x: bounds.x + padding,
                y: bounds.y + bounds.height
                    - total_scrollbar_height / 2.0
                    - width / 2.0,
                width: (bounds.width - scrollbar_y_width - 2.0 * padding)
                    .max(0.0),
                height: width,
            };

            let ratio = bounds.width / content_bounds.width;

            let scroller = if ratio >= 1.0 {
                None
            } else {
                // min width for easier grabbing with extra wide content
                let scroller_length = (scrollbar_bounds.width * ratio).max(2.0);
                let scroller_offset =
                    translation.x * ratio * scrollbar_bounds.width
                        / bounds.width;

                let scroller_bounds = Rectangle {
                    x: (scrollbar_bounds.x + scroller_offset).max(0.0),
                    y: bounds.y + bounds.height
                        - total_scrollbar_height / 2.0
                        - scroller_width / 2.0,
                    width: scroller_length,
                    height: scroller_width,
                };

                Some(internals::Scroller {
                    bounds: scroller_bounds,
                })
            };

            Some(internals::Scrollbar {
                total_bounds: total_scrollbar_bounds,
                bounds: scrollbar_bounds,
                scroller,
                alignment: horizontal.alignment,
                disabled: content_bounds.width <= bounds.width,
            })
        } else {
            None
        };

        Self {
            y: y_scrollbar,
            x: x_scrollbar,
        }
    }

    fn is_mouse_over(&self, cursor: mouse::Cursor) -> (bool, bool) {
        if let Some(cursor_position) = cursor.position() {
            (
                self.y
                    .as_ref()
                    .map(|scrollbar| scrollbar.is_mouse_over(cursor_position))
                    .unwrap_or(false),
                self.x
                    .as_ref()
                    .map(|scrollbar| scrollbar.is_mouse_over(cursor_position))
                    .unwrap_or(false),
            )
        } else {
            (false, false)
        }
    }

    fn is_y_disabled(&self) -> bool {
        self.y.map(|y| y.disabled).unwrap_or(false)
    }

    fn is_x_disabled(&self) -> bool {
        self.x.map(|x| x.disabled).unwrap_or(false)
    }

    fn grab_y_scroller(&self, cursor_position: Point) -> Option<f32> {
        let scrollbar = self.y?;
        let scroller = scrollbar.scroller?;

        if scrollbar.total_bounds.contains(cursor_position) {
            Some(if scroller.bounds.contains(cursor_position) {
                (cursor_position.y - scroller.bounds.y) / scroller.bounds.height
            } else {
                0.5
            })
        } else {
            None
        }
    }

    fn grab_x_scroller(&self, cursor_position: Point) -> Option<f32> {
        let scrollbar = self.x?;
        let scroller = scrollbar.scroller?;

        if scrollbar.total_bounds.contains(cursor_position) {
            Some(if scroller.bounds.contains(cursor_position) {
                (cursor_position.x - scroller.bounds.x) / scroller.bounds.width
            } else {
                0.5
            })
        } else {
            None
        }
    }

    fn active(&self) -> bool {
        self.y.is_some() || self.x.is_some()
    }
}

pub(super) mod internals {
    use crate::core::{Point, Rectangle};

    use super::Anchor;

    #[derive(Debug, Copy, Clone)]
    pub struct Scrollbar {
        pub total_bounds: Rectangle,
        pub bounds: Rectangle,
        pub scroller: Option<Scroller>,
        pub alignment: Anchor,
        pub disabled: bool,
    }

    impl Scrollbar {
        /// Returns whether the mouse is over the scrollbar or not.
        pub fn is_mouse_over(&self, cursor_position: Point) -> bool {
            self.total_bounds.contains(cursor_position)
        }

        /// Returns the y-axis scrolled percentage from the cursor position.
        pub fn scroll_percentage_y(
            &self,
            grabbed_at: f32,
            cursor_position: Point,
        ) -> f32 {
            if let Some(scroller) = self.scroller {
                let percentage = (cursor_position.y
                    - self.bounds.y
                    - scroller.bounds.height * grabbed_at)
                    / (self.bounds.height - scroller.bounds.height);

                match self.alignment {
                    Anchor::Start => percentage,
                    Anchor::End => 1.0 - percentage,
                }
            } else {
                0.0
            }
        }

        /// Returns the x-axis scrolled percentage from the cursor position.
        pub fn scroll_percentage_x(
            &self,
            grabbed_at: f32,
            cursor_position: Point,
        ) -> f32 {
            if let Some(scroller) = self.scroller {
                let percentage = (cursor_position.x
                    - self.bounds.x
                    - scroller.bounds.width * grabbed_at)
                    / (self.bounds.width - scroller.bounds.width);

                match self.alignment {
                    Anchor::Start => percentage,
                    Anchor::End => 1.0 - percentage,
                }
            } else {
                0.0
            }
        }
    }

    /// The handle of a [`Scrollbar`].
    #[derive(Debug, Clone, Copy)]
    pub struct Scroller {
        /// The bounds of the [`Scroller`].
        pub bounds: Rectangle,
    }
}

/// The possible status of a [`Scrollable`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The [`Scrollable`] can be interacted with.
    Active {
        /// Whether or not the horizontal scrollbar is disabled meaning the content isn't overflowing.
        is_horizontal_scrollbar_disabled: bool,
        /// Whether or not the vertical scrollbar is disabled meaning the content isn't overflowing.
        is_vertical_scrollbar_disabled: bool,
    },
    /// The [`Scrollable`] is being hovered.
    Hovered {
        /// Indicates if the horizontal scrollbar is being hovered.
        is_horizontal_scrollbar_hovered: bool,
        /// Indicates if the vertical scrollbar is being hovered.
        is_vertical_scrollbar_hovered: bool,
        /// Whether or not the horizontal scrollbar is disabled meaning the content isn't overflowing.
        is_horizontal_scrollbar_disabled: bool,
        /// Whether or not the vertical scrollbar is disabled meaning the content isn't overflowing.
        is_vertical_scrollbar_disabled: bool,
    },
    /// The [`Scrollable`] is being dragged.
    Dragged {
        /// Indicates if the horizontal scrollbar is being dragged.
        is_horizontal_scrollbar_dragged: bool,
        /// Indicates if the vertical scrollbar is being dragged.
        is_vertical_scrollbar_dragged: bool,
        /// Whether or not the horizontal scrollbar is disabled meaning the content isn't overflowing.
        is_horizontal_scrollbar_disabled: bool,
        /// Whether or not the vertical scrollbar is disabled meaning the content isn't overflowing.
        is_vertical_scrollbar_disabled: bool,
    },
}

/// The appearance of a scrollable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The [`container::Style`] of a scrollable.
    pub container: container::Style,
    /// The vertical [`Rail`] appearance.
    pub vertical_rail: Rail,
    /// The horizontal [`Rail`] appearance.
    pub horizontal_rail: Rail,
    /// The [`Background`] of the gap between a horizontal and vertical scrollbar.
    pub gap: Option<Background>,
    /// The appearance of the [`AutoScroll`] overlay.
    pub auto_scroll: AutoScroll,
}

/// The appearance of the scrollbar of a scrollable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rail {
    /// The [`Background`] of a scrollbar.
    pub background: Option<Background>,
    /// The [`Border`] of a scrollbar.
    pub border: Border,
    /// The appearance of the [`Scroller`] of a scrollbar.
    pub scroller: Scroller,
}

/// The appearance of the scroller of a scrollable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scroller {
    /// The [`Background`] of the scroller.
    pub background: Background,
    /// The [`Border`] of the scroller.
    pub border: Border,
}

/// The appearance of the autoscroll overlay of a scrollable.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutoScroll {
    /// The [`Background`] of the [`AutoScroll`] overlay.
    pub background: Background,
    /// The [`Border`] of the [`AutoScroll`] overlay.
    pub border: Border,
    /// Thje [`Shadow`] of the [`AutoScroll`] overlay.
    pub shadow: Shadow,
    /// The [`Color`] for the arrow icons of the [`AutoScroll`] overlay.
    pub icon: Color,
}

/// The theme catalog of a [`Scrollable`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class with the given status.
    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style;
}

/// A styling function for a [`Scrollable`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme, Status) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>, status: Status) -> Style {
        class(self, status)
    }
}

/// The default style of a [`Scrollable`].
pub fn default(theme: &Theme, status: Status) -> Style {
    let palette = theme.extended_palette();

    let scrollbar = Rail {
        background: Some(palette.background.weak.color.into()),
        border: border::rounded(2),
        scroller: Scroller {
            background: palette.background.strongest.color.into(),
            border: border::rounded(2),
        },
    };

    let auto_scroll = AutoScroll {
        background: palette.background.base.color.scale_alpha(0.9).into(),
        border: border::rounded(u32::MAX)
            .width(1)
            .color(palette.background.base.text.scale_alpha(0.8)),
        shadow: Shadow {
            color: Color::BLACK.scale_alpha(0.7),
            offset: Vector::ZERO,
            blur_radius: 2.0,
        },
        icon: palette.background.base.text.scale_alpha(0.8),
    };

    match status {
        Status::Active { .. } => Style {
            container: container::Style::default(),
            vertical_rail: scrollbar,
            horizontal_rail: scrollbar,
            gap: None,
            auto_scroll,
        },
        Status::Hovered {
            is_horizontal_scrollbar_hovered,
            is_vertical_scrollbar_hovered,
            ..
        } => {
            let hovered_scrollbar = Rail {
                scroller: Scroller {
                    background: palette.primary.strong.color.into(),
                    ..scrollbar.scroller
                },
                ..scrollbar
            };

            Style {
                container: container::Style::default(),
                vertical_rail: if is_vertical_scrollbar_hovered {
                    hovered_scrollbar
                } else {
                    scrollbar
                },
                horizontal_rail: if is_horizontal_scrollbar_hovered {
                    hovered_scrollbar
                } else {
                    scrollbar
                },
                gap: None,
                auto_scroll,
            }
        }
        Status::Dragged {
            is_horizontal_scrollbar_dragged,
            is_vertical_scrollbar_dragged,
            ..
        } => {
            let dragged_scrollbar = Rail {
                scroller: Scroller {
                    background: palette.primary.base.color.into(),
                    ..scrollbar.scroller
                },
                ..scrollbar
            };

            Style {
                container: container::Style::default(),
                vertical_rail: if is_vertical_scrollbar_dragged {
                    dragged_scrollbar
                } else {
                    scrollbar
                },
                horizontal_rail: if is_horizontal_scrollbar_dragged {
                    dragged_scrollbar
                } else {
                    scrollbar
                },
                gap: None,
                auto_scroll,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEWPORT: Rectangle = Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 100.0,
    };

    fn content(height: f32) -> Rectangle {
        Rectangle {
            x: 0.0,
            y: 0.0,
            width: 100.0,
            height,
        }
    }

    fn offset_y(state: &State, content_bounds: Rectangle) -> f32 {
        state
            .offset_y
            .absolute(VIEWPORT.height, content_bounds.height)
    }

    #[test]
    fn wheel_scroll_is_animated() {
        let content_bounds = content(1000.0);
        let start = Instant::now();
        let mut state = State::default();

        let moved = state.scroll_wheel(
            Vector::new(0.0, wheel_step(VIEWPORT.height)),
            VIEWPORT,
            content_bounds,
            start,
        );
        assert!(moved);

        let mut now = start;
        let mut frames = 0;
        while state.smooth_tick(now, VIEWPORT, content_bounds) {
            now += Duration::from_millis(16);
            frames += 1;
            assert!(frames < 100, "animation did not settle");
        }

        assert!(offset_y(&state, content_bounds) > 0.0);
        assert!(
            (offset_y(&state, content_bounds) - wheel_step(VIEWPORT.height))
                .abs()
                < 0.001
        );
        assert!(state.smooth_scroll.is_none());
    }

    #[test]
    fn animation_advances_once_per_frame() {
        let content_bounds = content(1000.0);
        let start = Instant::now();
        let mut state = State::default();

        let moved = state.scroll_wheel(
            Vector::new(0.0, wheel_step(VIEWPORT.height)),
            VIEWPORT,
            content_bounds,
            start,
        );
        assert!(moved);

        // A redraw event can be processed more than once per frame (for
        // example, when publishing a message rebuilds the user interface).
        assert!(state.smooth_tick(start, VIEWPORT, content_bounds));
        let after_first_frame = offset_y(&state, content_bounds);

        assert!(state.smooth_tick(start, VIEWPORT, content_bounds));
        assert_eq!(offset_y(&state, content_bounds), after_first_frame);
    }

    #[test]
    fn wheel_scroll_does_not_move_past_the_edge() {
        let content_bounds = content(1000.0);
        let start = Instant::now();
        let mut state = State::default();

        let moved = state.scroll_wheel(
            Vector::new(0.0, -wheel_step(VIEWPORT.height)),
            VIEWPORT,
            content_bounds,
            start,
        );

        assert!(!moved);
        assert!(state.smooth_scroll.is_none());
    }

    #[test]
    fn precise_scroll_applies_immediately() {
        let content_bounds = content(1000.0);
        let start = Instant::now();
        let mut state = State::default();
        let delta = Vector::new(0.0, 10.0);

        assert!(state.scroll_precise(delta, VIEWPORT, content_bounds, start));
        assert!(
            (offset_y(&state, content_bounds) - delta.y * PRECISE_SCROLL_SCALE)
                .abs()
                < 0.001
        );
    }

    #[test]
    fn precise_scroll_glides_after_the_gesture_ends() {
        let content_bounds = content(10000.0);
        let start = Instant::now();
        let mut state = State::default();

        // Sample a couple of precise deltas to build up velocity.
        let mut now = start;
        for _ in 0..2 {
            assert!(state.scroll_precise(
                Vector::new(0.0, 10.0),
                VIEWPORT,
                content_bounds,
                now,
            ));
            now += Duration::from_millis(10);
        }

        let before_glide = offset_y(&state, content_bounds);

        // Letting go of the touchpad reports the end of the gesture on
        // Wayland (the `axis_stop` event).
        let _ = state.end_precise_scroll(now, VIEWPORT, content_bounds);

        let mut frames = 0;
        while state.smooth_tick(now, VIEWPORT, content_bounds) {
            now += Duration::from_millis(16);
            frames += 1;
            assert!(frames < 1000, "momentum did not settle");
        }

        assert!(
            offset_y(&state, content_bounds) > before_glide,
            "content should glide after the gesture ends"
        );
    }

    #[test]
    fn precise_scroll_stops_at_the_edge() {
        let content_bounds = content(200.0);
        let start = Instant::now();
        let mut state = State::default();

        let mut now = start;
        for _ in 0..2 {
            let _ = state.scroll_precise(
                Vector::new(0.0, 50.0),
                VIEWPORT,
                content_bounds,
                now,
            );
            now += Duration::from_millis(10);
        }

        let _ = state.end_precise_scroll(now, VIEWPORT, content_bounds);

        let mut frames = 0;
        while state.smooth_tick(now, VIEWPORT, content_bounds) {
            now += Duration::from_millis(16);
            frames += 1;
            assert!(frames < 1000, "momentum did not settle");
        }

        assert!((offset_y(&state, content_bounds) - 100.0).abs() < 0.001);
    }

    #[test]
    fn precise_scroll_does_not_glide_after_a_pause() {
        let content_bounds = content(10000.0);
        let start = Instant::now();
        let mut state = State::default();

        let mut now = start;
        for _ in 0..2 {
            let _ = state.scroll_precise(
                Vector::new(0.0, 10.0),
                VIEWPORT,
                content_bounds,
                now,
            );
            now += Duration::from_millis(10);
        }

        let before_end = offset_y(&state, content_bounds);

        // The fingers were already resting before the gesture ended.
        let end = now + Duration::from_millis(500);
        let _ = state.end_precise_scroll(end, VIEWPORT, content_bounds);
        assert_eq!(offset_y(&state, content_bounds), before_end);

        let mut frames = 0;
        while state.smooth_tick(end, VIEWPORT, content_bounds) {
            frames += 1;
            assert!(frames < 100, "no momentum should start after a pause");
        }

        assert_eq!(offset_y(&state, content_bounds), before_end);
    }

    #[test]
    fn precise_scroll_does_not_glide_without_a_gesture_end() {
        let content_bounds = content(10000.0);
        let start = Instant::now();
        let mut state = State::default();

        let mut now = start;
        for _ in 0..2 {
            let _ = state.scroll_precise(
                Vector::new(0.0, 10.0),
                VIEWPORT,
                content_bounds,
                now,
            );
            now += Duration::from_millis(10);
        }

        let after_input = offset_y(&state, content_bounds);

        // Ticking frames without a gesture end must not move the content on
        // its own.
        for _ in 0..60 {
            if !state.smooth_tick(now, VIEWPORT, content_bounds) {
                break;
            }
            now += Duration::from_millis(16);
        }

        assert_eq!(offset_y(&state, content_bounds), after_input);
    }
}
