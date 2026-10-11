// Shows a subsurface with a 1x1 px red buffer, stretch to window size

use cctk::sctk::reexports::{
    client::{Connection, Proxy},
    protocols::xdg::shell::client::xdg_positioner::{Anchor, Gravity},
};

use iced::platform_specific::shell::commands::subsurface::get_subsurface;
use iced::{
    event::wayland::Event as WaylandEvent,
    platform_specific::{
        runtime::wayland::subsurface::SctkSubsurfaceSettings,
        shell::subsurface_widget::{self, SubsurfaceBuffer},
    },
    widget::{button, column, text, text_input},
    window::{self, Id, Settings},
    Element, Length, Subscription, Task,
};

mod subsurface_container;
mod wayland;

fn main() -> iced::Result {
    iced::daemon(
        SubsurfaceApp::new,
        SubsurfaceApp::update,
        SubsurfaceApp::view,
    )
    .title(SubsurfaceApp::title)
    .subscription(SubsurfaceApp::subscription)
    .run()
}

#[derive(Debug, Clone, Default)]
struct SubsurfaceApp {
    text: String,
    counter: u32,
    connection: Option<Connection>,
    red_buffer: Option<SubsurfaceBuffer>,
    green_buffer: Option<SubsurfaceBuffer>,
}

#[derive(Debug, Clone)]
pub enum Message {
    WaylandEvent(WaylandEvent),
    Wayland(wayland::Event),
    Pressed(&'static str),
    Id(Id),
    Inc,
    Text(String),
}

impl SubsurfaceApp {
    fn new() -> (SubsurfaceApp, Task<Message>) {
        (
            SubsurfaceApp {
                ..SubsurfaceApp::default()
            },
            iced::window::open(Settings {
                ..Default::default()
            })
            .1
            .map(Message::Id),
        )
    }

    fn title(&self, _id: window::Id) -> String {
        String::from("SubsurfaceApp")
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::WaylandEvent(evt) => match evt {
                WaylandEvent::Output(_evt, output) => {
                    if self.connection.is_none() {
                        if let Some(backend) = output.backend().upgrade() {
                            self.connection = Some(Connection::from_backend(backend));
                        }
                    }
                }
                _ => {}
            },
            Message::Wayland(evt) => match evt {
                wayland::Event::RedBuffer(buffer) => {
                    self.red_buffer = Some(buffer);
                }
                wayland::Event::GreenBuffer(buffer) => {
                    self.green_buffer = Some(buffer);
                }
            },
            Message::Pressed(side) => println!("{side} surface pressed"),
            Message::Id(id) => {
                return get_subsurface(SctkSubsurfaceSettings {
                    id: window::Id::unique(),
                    parent: id,
                    loc: iced::Point::new(100., 200.),
                    size: Some(iced::Size::new(100., 100.)),
                    z: 1000,
                    steal_keyboard_focus: false,
                    gravity: Gravity::BottomRight,
                    input_zone: None,
                    offset: (0, 0),
                });
            }
            Message::Inc => {
                self.counter += 1;
            }
            Message::Text(s) => {
                self.text = s;
            }
        }
        Task::none()
    }

    fn view(&self, id: window::Id) -> Element<Message> {
        if let Some((red_buffer, green_buffer)) =
            self.red_buffer.iter().zip(self.green_buffer.iter()).next()
        {
            Element::new(column![
                iced::widget::row![
                    iced::widget::button(
                        subsurface_container::SubsurfaceContainer::new()
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .push(
                                subsurface_widget::Subsurface::new(red_buffer.clone())
                                    .width(Length::Fill)
                                    .height(Length::Fill)
                                    .z(0)
                            )
                            .push(
                                subsurface_widget::Subsurface::new(green_buffer.clone())
                                    .width(Length::Fixed(1920.))
                                    .height(Length::Fixed(200.))
                                    .z(1)
                            )
                            .push(
                                subsurface_widget::Subsurface::new(red_buffer.clone())
                                    .width(Length::Fill)
                                    .height(Length::Fixed(100.))
                                    .z(2)
                            )
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .on_press(Message::Pressed("left")),
                    iced::widget::button(
                        subsurface_widget::Subsurface::new(red_buffer.clone())
                            .width(Length::Fill)
                            .height(Length::Fill)
                    )
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .on_press(Message::Pressed("right"))
                ],
                text_input("asdf", &self.text).on_input(Message::Text)
            ])
        } else {
            Element::new(text("No subsurface"))
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![iced::event::listen_with(|evt, _, _| {
            if let iced::Event::PlatformSpecific(iced::event::PlatformSpecific::Wayland(
                WaylandEvent::Output(evt, output),
            )) = evt
            {
                Some(Message::WaylandEvent(WaylandEvent::Output(evt, output)))
            } else {
                None
            }
        })];
        if let Some(connection) = &self.connection {
            subscriptions.push(wayland::subscription(connection).map(Message::Wayland));
        }
        Subscription::batch(subscriptions)
    }
}
