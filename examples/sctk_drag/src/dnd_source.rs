use iced::id::Id;
use iced::widget::container;
use iced::Element;
use iced::Widget as IcedWidget;
use iced::{
    clipboard::dnd::{DndAction, DndEvent, SourceEvent},
    mouse, overlay, Event, Length, Point, Rectangle,
};
use iced_core::{
    layout, renderer,
    widget::{tree, Meta, Tree},
    Shell,
};
use iced_core::{Layout, Widget};

pub fn dnd_source<
    'a,
    Message: 'static,
    AppMessage: 'static,
    D: iced::clipboard::mime::AsMimeTypes + Send + 'static,
>(
    child: impl IcedWidget<Message> + 'a,
) -> DndSource<'a, Message, AppMessage, D> {
    DndSource::new(child)
}

pub struct DndSource<'a, Message, AppMessage, D> {
    id: Id,
    action: DndAction,
    container: Element<'a, Message>,
    drag_content: Option<Box<dyn Fn() -> D>>,
    drag_icon: Option<Box<dyn Fn() -> (Element<'static, AppMessage>, tree::State)>>,
    drag_threshold: f32,
    _phantom: std::marker::PhantomData<AppMessage>,
}

impl<
        'a,
        Message: 'static,
        AppMessage: 'static,
        D: iced::clipboard::mime::AsMimeTypes + std::marker::Send + 'static,
    > DndSource<'a, Message, AppMessage, D>
{
    pub fn new(child: impl IcedWidget<Message> + 'a) -> Self {
        Self {
            id: Id::unique(),
            action: DndAction::Copy | DndAction::Move,
            container: Element::new(container(child)),
            drag_content: None,
            drag_icon: None,
            drag_threshold: 8.0,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn with_id(child: impl IcedWidget<Message> + 'a, id: Id) -> Self {
        Self {
            id,
            action: DndAction::Copy | DndAction::Move,
            container: Element::new(container(child)),
            drag_content: None,
            drag_icon: None,
            drag_threshold: 8.0,
            _phantom: std::marker::PhantomData,
        }
    }

    #[must_use]
    pub fn action(mut self, action: DndAction) -> Self {
        self.action = action;
        self
    }

    #[must_use]
    pub fn drag_content(mut self, f: impl Fn() -> D + 'static) -> Self {
        self.drag_content = Some(Box::new(f));
        self
    }

    #[must_use]
    pub fn drag_icon(
        mut self,
        f: impl Fn() -> (Element<'static, AppMessage>, tree::State) + 'static,
    ) -> Self {
        self.drag_icon = Some(Box::new(f));
        self
    }

    #[must_use]
    pub fn drag_threshold(mut self, threshold: f32) -> Self {
        self.drag_threshold = threshold;
        self
    }

    pub fn start_dnd(&self, shell: &mut Shell<'_, Message>) {
        let Some(content) = self.drag_content.as_ref().map(|f| f()) else {
            return;
        };
        let icon = self.drag_icon.as_ref().map(|f| {
            let (icon, state) = f();
            let icon = Element::new(icon._map(|_| ()));
            iced_core::clipboard::IconSurface::new(icon, state, iced_core::Vector::ZERO).upcast()
        });
        shell.start_dnd(
            false,
            Some(iced_core::clipboard::DndSource::Widget(self.id.clone())),
            icon,
            Box::new(content),
            self.action,
        );
    }
}

impl<'a, Message, AppMessage, D> Meta for DndSource<'a, Message, AppMessage, D> {}

impl<
        'a,
        Message: 'static,
        AppMessage: 'static,
        D: iced::clipboard::mime::AsMimeTypes + std::marker::Send + 'static,
    > Widget<Message, iced::Theme, iced::Renderer> for DndSource<'a, Message, AppMessage, D>
{
    fn tag(&self) -> iced_core::widget::tree::Tag {
        tree::Tag::of::<State>()
    }

    fn diff(&mut self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_mut(&mut self.container));
    }

    fn state(&self) -> iced_core::widget::tree::State {
        tree::State::new(State::new())
    }

    fn size(&self) -> iced_core::Size<Length> {
        self.container.as_widget().size()
    }

    fn layout(&mut self, tree: &mut Tree, renderer: &iced::Renderer, limits: &layout::Limits) {
        self.container
            .layout(&mut tree.children[0], renderer, limits);
        tree.size = tree.children[0].size;
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
        operation: &mut dyn iced_core::widget::Operation<()>,
    ) {
        operation.pre_operation(Some(&self.id));
        operation.custom(Some(&self.id), layout.bounds(), &mut tree.state);
        operation.container(Some(&self.id), layout.bounds(), viewport);
        operation.traverse(&mut |operation| {
            self.container
                .operate(&mut tree.children[0], layout, viewport, renderer, operation)
        });
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.container.update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            shell,
            viewport,
        );
        if shell.is_event_captured() {
            return;
        }

        let state = tree.state.downcast_mut::<State>();

        match event {
            Event::Mouse(mouse_event) => match mouse_event {
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if let Some(position) = cursor.position() {
                        if !state.hovered {
                            return;
                        }

                        state.left_pressed_position = Some(position);
                        shell.capture_event();
                        return;
                    }
                }
                mouse::Event::ButtonReleased(mouse::Button::Left)
                    if state.left_pressed_position.is_some() =>
                {
                    state.left_pressed_position = None;
                    shell.capture_event();
                    return;
                }
                mouse::Event::CursorMoved { .. } => {
                    if let Some(position) = cursor.position() {
                        if state.hovered {
                            // We ignore motion if we do not possess drag content by now.
                            if self.drag_content.is_none() {
                                state.left_pressed_position = None;
                                return;
                            }
                            if let Some(left_pressed_position) = state.left_pressed_position {
                                if position.distance(left_pressed_position) > self.drag_threshold {
                                    self.start_dnd(shell);
                                    state.is_dragging = true;
                                    state.left_pressed_position = None;
                                }
                            }
                            if !cursor.is_over(layout.bounds()) {
                                state.hovered = false;

                                return;
                            }
                        } else if cursor.is_over(layout.bounds()) {
                            state.hovered = true;
                        }
                        shell.capture_event();
                        return;
                    }
                }
                _ => return,
            },
            Event::Dnd(DndEvent::Source(SourceEvent::Cancelled | SourceEvent::Finished)) => {
                if state.is_dragging {
                    state.is_dragging = false;
                    shell.capture_event();
                    return;
                }
                return;
            }
            _ => return,
        }
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout,
        cursor_position: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();
        if state.is_dragging {
            return mouse::Interaction::Grabbing;
        }
        self.container.mouse_interaction(
            &tree.children[0],
            layout,
            cursor_position,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        renderer_style: &renderer::Style,
        layout: Layout,
        cursor_position: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.container.draw(
            &tree.children[0],
            renderer,
            theme,
            renderer_style,
            layout,
            cursor_position,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: iced::Vector,
        window: iced::Size,
    ) -> Vec<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.container
            .overlay(tree, layout, renderer, viewport, translation, window)
    }

    fn drag_destinations(
        &self,
        state: &Tree,
        layout: Layout,
        renderer: &iced::Renderer,
        dnd_rectangles: &mut iced_core::clipboard::DndDestinationRectangles,
    ) {
        self.container
            .drag_destinations(&state.children[0], layout, renderer, dnd_rectangles);
    }

    fn id(&self) -> Option<Id> {
        Some(self.id.clone())
    }

    fn set_id(&mut self, id: Id) {
        self.id = id;
    }
}

impl<
        'a,
        Message: 'static,
        AppMessage: 'static,
        D: iced::clipboard::mime::AsMimeTypes + std::marker::Send + 'static,
    > From<DndSource<'a, Message, AppMessage, D>> for Element<'a, Message>
{
    fn from(e: DndSource<'a, Message, AppMessage, D>) -> Element<'a, Message> {
        Element::new(e)
    }
}

/// Local state of the [`MouseListener`].
#[derive(Debug, Default)]
struct State {
    hovered: bool,
    left_pressed_position: Option<Point>,
    is_dragging: bool,
}

impl State {
    fn new() -> Self {
        Self::default()
    }
}
