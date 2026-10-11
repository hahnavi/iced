//! Distribute content vertically.
use iced::core::alignment::{self, Alignment};
use iced::core::event::Event;
use iced::core::layout;
use iced::core::mouse;
use iced::core::overlay;
use iced::core::renderer;
use iced::core::widget::{self, tree, Operation, Tree};
use iced::core::{
    Element, Layout, Length, Padding, Pixels, Rectangle, Shell, Size, Vector, Widget,
};

/// A container that distributes its contents vertically.
///
/// # Example
/// ```no_run
/// # mod iced { pub mod widget { pub use iced_widget::*; } }
/// # pub type State = ();
/// # pub type Element<'a, Message> = iced_widget::core::Element<'a, Message, iced_widget::Theme, iced_widget::Renderer>;
/// use iced::widget::{button, SubsurfaceContainer};
///
/// #[derive(Debug, Clone)]
/// enum Message {
///     // ...
/// }
///
/// fn view(state: &State) -> Element<'_, Message> {
///     SubsurfaceContainer![
///         "I am on top!",
///         button("I am in the center!"),
///         "I am below.",
///     ].into()
/// }
/// ```
#[allow(missing_debug_implementations)]
pub struct SubsurfaceContainer<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    spacing: f32,
    padding: Padding,
    width: Length,
    height: Length,
    max_width: f32,
    align: Alignment,
    clip: bool,
    children: Vec<Element<'a, Message, Theme, Renderer>>,
}

impl<'a, Message, Theme, Renderer> SubsurfaceContainer<'a, Message, Theme, Renderer>
where
    Renderer: iced::core::Renderer,
{
    /// Creates an empty [`SubsurfaceContainer`].
    pub fn new() -> Self {
        Self::from_vec(Vec::new())
    }

    /// Creates a [`SubsurfaceContainer`] with the given capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::from_vec(Vec::with_capacity(capacity))
    }

    /// Creates a [`SubsurfaceContainer`] with the given elements.
    pub fn with_children(
        children: impl IntoIterator<Item = Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        let iterator = children.into_iter();

        Self::with_capacity(iterator.size_hint().0).extend(iterator)
    }

    /// Creates a [`SubsurfaceContainer`] from an already allocated [`Vec`].
    ///
    /// Keep in mind that the [`SubsurfaceContainer`] will not inspect the [`Vec`], which means
    /// it won't automatically adapt to the sizing strategy of its contents.
    ///
    /// If any of the children have a [`Length::Fill`] strategy, you will need to
    /// call [`SubsurfaceContainer::width`] or [`SubsurfaceContainer::height`] accordingly.
    pub fn from_vec(children: Vec<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            spacing: 0.0,
            padding: Padding::ZERO,
            width: Length::Shrink,
            height: Length::Shrink,
            max_width: f32::INFINITY,
            align: Alignment::Start,
            clip: false,
            children,
        }
    }

    /// Sets the vertical spacing _between_ elements.
    ///
    /// Custom margins per element do not exist in iced. You should use this
    /// method instead! While less flexible, it helps you keep spacing between
    /// elements consistent.
    pub fn spacing(mut self, amount: impl Into<Pixels>) -> Self {
        self.spacing = amount.into().0;
        self
    }

    /// Sets the [`Padding`] of the [`SubsurfaceContainer`].
    pub fn padding<P: Into<Padding>>(mut self, padding: P) -> Self {
        self.padding = padding.into();
        self
    }

    /// Sets the width of the [`SubsurfaceContainer`].
    pub fn width(mut self, width: impl Into<Length>) -> Self {
        self.width = width.into();
        self
    }

    /// Sets the height of the [`SubsurfaceContainer`].
    pub fn height(mut self, height: impl Into<Length>) -> Self {
        self.height = height.into();
        self
    }

    /// Sets the maximum width of the [`SubsurfaceContainer`].
    pub fn max_width(mut self, max_width: impl Into<Pixels>) -> Self {
        self.max_width = max_width.into().0;
        self
    }

    /// Sets the horizontal alignment of the contents of the [`SubsurfaceContainer`] .
    pub fn align_x(mut self, align: impl Into<alignment::Horizontal>) -> Self {
        self.align = Alignment::from(align.into());
        self
    }

    /// Sets whether the contents of the [`SubsurfaceContainer`] should be clipped on
    /// overflow.
    pub fn clip(mut self, clip: bool) -> Self {
        self.clip = clip;
        self
    }

    /// Adds an element to the [`SubsurfaceContainer`].
    pub fn push(mut self, child: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        let child = child.into();
        let child_size = child.size();

        self.width = self.width.cross(child_size.width);
        self.height = self.height.stack(child_size.height);

        self.children.push(child);
        self
    }

    /// Adds an element to the [`SubsurfaceContainer`], if `Some`.
    pub fn push_maybe(
        self,
        child: Option<impl Into<Element<'a, Message, Theme, Renderer>>>,
    ) -> Self {
        if let Some(child) = child {
            self.push(child)
        } else {
            self
        }
    }

    /// Extends the [`SubsurfaceContainer`] with the given children.
    pub fn extend(
        self,
        children: impl IntoIterator<Item = Element<'a, Message, Theme, Renderer>>,
    ) -> Self {
        children.into_iter().fold(self, Self::push)
    }
}

impl<'a, Message, Renderer> Default for SubsurfaceContainer<'a, Message, Renderer>
where
    Renderer: iced::core::Renderer,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, Message, Theme, Renderer> widget::Meta
    for SubsurfaceContainer<'a, Message, Theme, Renderer>
{
}

impl<'a, Message, Theme, Renderer: iced::core::Renderer>
    FromIterator<Element<'a, Message, Theme, Renderer>>
    for SubsurfaceContainer<'a, Message, Theme, Renderer>
{
    fn from_iter<T: IntoIterator<Item = Element<'a, Message, Theme, Renderer>>>(iter: T) -> Self {
        Self::with_children(iter)
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for SubsurfaceContainer<'a, Message, Theme, Renderer>
where
    Renderer: iced::core::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<layout::flex::Cache>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(layout::flex::Cache::default())
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(self.children.as_mut_slice());
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: self.width,
            height: self.height,
        }
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        let limits = layout::Limits::with_flags(
            limits.min,
            Size::new(limits.max.width.min(self.max_width), limits.max.height),
            limits.compression,
            limits.infinite,
        );
        let size = layout::flex::resolve(
            layout::flex::Axis::Vertical,
            renderer,
            &limits,
            self.width,
            self.height,
            self.padding,
            self.spacing,
            self.align,
            &mut tree.children,
            &mut self.children,
            tree.state.downcast_mut::<layout::flex::Cache>(),
        );
        tree.size = size;
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        operation.container(None, layout.bounds(), viewport);
        operation.traverse(&mut |operation| {
            self.children
                .iter_mut()
                .zip(layout.iter_mut(&mut tree.children))
                .for_each(|(child, (child_layout, state))| {
                    child.operate(state, child_layout, viewport, renderer, operation);
                });
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.children
            .iter_mut()
            .zip(layout.iter_mut(&mut tree.children))
            .for_each(|(child, (child_layout, state))| {
                child.update(
                    state,
                    event,
                    child_layout,
                    cursor,
                    renderer,
                    shell,
                    viewport,
                );
            });
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.children
            .iter()
            .zip(layout.iter(&tree.children))
            .map(|(child, (child_layout, state))| {
                child.mouse_interaction(state, child_layout, cursor, viewport, renderer)
            })
            .max()
            .unwrap_or_default()
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        if let Some(clipped_viewport) = layout.bounds().intersection(viewport) {
            let viewport = if self.clip {
                &clipped_viewport
            } else {
                viewport
            };

            for (child, (child_layout, state)) in self
                .children
                .iter()
                .zip(layout.iter(&tree.children))
                .filter(|(_, (child_layout, _))| child_layout.bounds().intersects(viewport))
            {
                child.draw(
                    state,
                    renderer,
                    theme,
                    style,
                    child_layout,
                    cursor,
                    viewport,
                );
            }
        }
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
        window: Size,
    ) -> Vec<overlay::Element<'b, Message, Theme, Renderer>> {
        overlay::from_children(
            &mut self.children,
            tree,
            layout,
            renderer,
            viewport,
            translation,
            window,
        )
    }

    #[cfg(feature = "a11y")]
    /// get the a11y nodes for the widget
    fn a11y_nodes(
        &self,
        layout: Layout,
        state: &Tree,
        cursor: mouse::Cursor,
    ) -> iced_accessibility::A11yTree {
        use iced_accessibility::A11yTree;
        A11yTree::join(self.children.iter().zip(layout.iter(&state.children)).map(
            |(child, (child_layout, child_state))| {
                child.a11y_nodes(child_layout, child_state, cursor)
            },
        ))
    }

    fn drag_destinations(
        &self,
        state: &Tree,
        layout: Layout,
        renderer: &Renderer,
        dnd_rectangles: &mut iced::core::clipboard::DndDestinationRectangles,
    ) {
        for (child, (child_layout, child_state)) in
            self.children.iter().zip(layout.iter(&state.children))
        {
            child.drag_destinations(child_state, child_layout, renderer, dnd_rectangles);
        }
    }
}

impl<'a, Message, Theme, Renderer> From<SubsurfaceContainer<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::core::Renderer + 'a,
{
    fn from(SubsurfaceContainer: SubsurfaceContainer<'a, Message, Theme, Renderer>) -> Self {
        Self::new(SubsurfaceContainer)
    }
}
