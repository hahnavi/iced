#![allow(missing_docs)]

use iced_test::Simulator;
use iced_widget::core::{self, Element, Length, Pixels, Size, Widget, widget::Id};
use iced_widget::{Renderer, Theme, container, list, text};

fn view<'a>(content: &'a list::Content<f32>) -> impl Widget<(), Theme, Renderer> + 'a {
    list::list(
        content,
        |index, height| -> Element<'_, (), Theme, Renderer> {
            Element::new(
                container(text("row"))
                    .width(Length::Fill)
                    .height(Pixels(*height))
                    .id(Id::from(format!("row-{index}"))),
            )
        },
    )
    .spacing(10)
}

fn row_y(simulator: &mut Simulator<'_, (), Theme, Renderer>, index: usize) -> f32 {
    simulator
        .find(Id::from(format!("row-{index}")))
        .expect("find list row")
        .bounds()
        .y
}

#[test]
fn spacing_and_updated_item_heights_keep_visible_rows_aligned() {
    let content = list::Content::with_items(vec![20.0, 30.0, 40.0]);
    let mut updated_content: Option<list::Content<f32>>;
    let mut simulator = Simulator::with_size(
        core::Settings::default(),
        Size::new(200.0, 200.0),
        view(&content),
    );
    simulator.draw(&Theme::Light);

    assert_eq!(row_y(&mut simulator, 0), 0.0);
    assert_eq!(row_y(&mut simulator, 1), 30.0);
    assert_eq!(row_y(&mut simulator, 2), 70.0);

    updated_content = Some(content.clone());
    let updated_content = updated_content.as_mut().expect("clone content");
    *updated_content.get_mut(0).expect("get first row") = 50.0;

    simulator = simulator.rebuild(view(updated_content));
    simulator.draw(&Theme::Light);

    assert_eq!(row_y(&mut simulator, 1), 60.0);
    assert_eq!(row_y(&mut simulator, 2), 100.0);
}

#[test]
fn queued_mutations_of_removed_items_are_ignored() {
    let content = list::Content::with_items(vec![20.0, 30.0]);
    let mut removed_after_update: Option<list::Content<f32>>;
    let mut simulator = Simulator::with_size(
        core::Settings::default(),
        Size::new(200.0, 200.0),
        view(&content),
    );
    simulator.draw(&Theme::Light);

    removed_after_update = Some(content.clone());
    let updated = removed_after_update.as_mut().expect("clone content");
    *updated.get_mut(1).expect("get last row") = 40.0;
    let _ = updated.remove(1);

    simulator = simulator.rebuild(view(updated));
    simulator.draw(&Theme::Light);
    assert_eq!(row_y(&mut simulator, 0), 0.0);

    let content = list::Content::with_items(vec![20.0]);
    let mut removed_after_push: Option<list::Content<f32>>;
    let mut simulator = Simulator::with_size(
        core::Settings::default(),
        Size::new(200.0, 200.0),
        view(&content),
    );
    simulator.draw(&Theme::Light);

    removed_after_push = Some(content.clone());
    let updated = removed_after_push.as_mut().expect("clone content");
    updated.push(40.0);
    let _ = updated.remove(1);

    simulator = simulator.rebuild(view(updated));
    simulator.draw(&Theme::Light);
    assert_eq!(row_y(&mut simulator, 0), 0.0);
}
