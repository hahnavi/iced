use crate::layout;
use crate::mouse;
use crate::overlay;
use crate::renderer;
use crate::widget;
use crate::widget::tree::{self, Tree};
use crate::{Event, Layout, Length, Rectangle, Shell, Size, Vector, Widget};

/// A generic [`Widget`].
///
/// It is useful to build composable user interfaces that do not leak
/// implementation details in their __view logic__.
///
/// If you have any [`Widget`], you should be able to use [`Widget::boxed`](Widget::_boxed)
/// to turn it into an [`Element`].
pub struct Element<'a, Message, Theme, Renderer> {
    widget: Box<dyn Widget<Message, Theme, Renderer> + 'a>,
}

impl<'a, Message, Theme, Renderer> Element<'a, Message, Theme, Renderer> {
    /// Creates a new [`Element`] containing the given [`Widget`].
    pub fn new(widget: impl Widget<Message, Theme, Renderer> + 'a) -> Self {
        Self {
            widget: Box::new(widget),
        }
    }

    /// Returns a reference to the wrapped [`Widget`].
    pub fn as_widget(&self) -> &dyn Widget<Message, Theme, Renderer> {
        self.widget.as_ref()
    }

    /// Returns a mutable reference to the wrapped [`Widget`].
    pub fn as_widget_mut(&mut self) -> &mut dyn Widget<Message, Theme, Renderer> {
        self.widget.as_mut()
    }
}

impl<'a, Message, Theme, Renderer> widget::Meta for Element<'a, Message, Theme, Renderer> {
    fn is_void(&self) -> bool {
        self.widget.is_void()
    }
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Element<'a, Message, Theme, Renderer>
{
    fn size(&self) -> Size<Length> {
        self.widget.size()
    }

    fn tag(&self) -> tree::Tag {
        self.widget.tag()
    }

    fn state(&self) -> tree::State {
        self.widget.state()
    }

    fn diff(&mut self, tree: &mut Tree) {
        self.widget.diff(tree);
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &Renderer, limits: &layout::Limits) {
        self.widget.layout(tree, renderer, limits);
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
        self.widget
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.widget
            .operate(tree, layout, viewport, renderer, operation);
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
        self.widget
            .update(tree, event, layout, cursor, renderer, shell, viewport);
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.widget
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
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
        self.widget
            .overlay(tree, layout, renderer, viewport, translation, window)
    }

    fn _boxed<'b>(self) -> Element<'b, Message, Theme, Renderer>
    where
        Self: Sized + 'b,
    {
        self
    }

    fn id(&self) -> Option<widget::Id> {
        self.widget.id()
    }

    fn set_id(&mut self, id: widget::Id) {
        self.widget.set_id(id);
    }

    #[cfg(feature = "a11y")]
    fn a11y_nodes(
        &self,
        layout: Layout,
        state: &Tree,
        cursor: mouse::Cursor,
    ) -> iced_accessibility::A11yTree {
        self.widget.a11y_nodes(layout, state, cursor)
    }

    fn drag_destinations(
        &self,
        state: &Tree,
        layout: Layout,
        renderer: &Renderer,
        dnd_rectangles: &mut crate::clipboard::DndDestinationRectangles,
    ) {
        self.widget
            .drag_destinations(state, layout, renderer, dnd_rectangles);
    }
}
