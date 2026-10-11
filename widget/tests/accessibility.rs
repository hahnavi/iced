//! Accessibility tree regression coverage.

#![cfg(feature = "a11y")]

use iced_widget::core::{Layout, Size, Widget, mouse, widget::Tree};
use iced_widget::{Renderer, Theme, button, checkbox, container, stack, text, toggler};

#[test]
fn button_includes_its_accessible_text_child() {
    let mut button = button(text("Activate")).on_press(());
    let mut state = Tree::new::<(), Theme, Renderer>(&button);

    <_ as Widget<(), Theme, Renderer>>::diff(&mut button, &mut state);

    let tree = <_ as Widget<(), Theme, Renderer>>::a11y_nodes(
        &button,
        Layout::new(Size::new(120.0, 32.0)),
        &state,
        mouse::Cursor::Unavailable,
    );

    assert_eq!(tree.root().len(), 1);
    assert_eq!(tree.children().len(), 1);
}

#[test]
fn labeled_checkbox_exposes_a_label_node() {
    let mut checkbox = checkbox(true).label("Enable").on_toggle(|_| ());
    let mut state = Tree::new::<(), Theme, Renderer>(&checkbox);

    <_ as Widget<(), Theme, Renderer>>::diff(&mut checkbox, &mut state);

    let tree = <_ as Widget<(), Theme, Renderer>>::a11y_nodes(
        &checkbox,
        Layout::new(Size::new(140.0, 24.0)),
        &state,
        mouse::Cursor::Unavailable,
    );

    assert_eq!(tree.root().len(), 1);
    assert_eq!(tree.children().len(), 1);
    assert_eq!(
        tree.root()[0].node().toggled(),
        Some(iced_accessibility::accesskit::Toggled::True)
    );
    assert!(!tree.root()[0].node().is_disabled());
}

#[test]
fn disabled_switch_exposes_its_toggled_state() {
    let mut toggler = toggler(true).label("Enable");
    let mut state = Tree::new::<(), Theme, Renderer>(&toggler);

    <_ as Widget<(), Theme, Renderer>>::diff(&mut toggler, &mut state);

    let tree = <_ as Widget<(), Theme, Renderer>>::a11y_nodes(
        &toggler,
        Layout::new(Size::new(140.0, 24.0)),
        &state,
        mouse::Cursor::Unavailable,
    );

    assert_eq!(tree.root().len(), 1);
    assert_eq!(tree.children().len(), 1);
    assert_eq!(
        tree.root()[0].node().toggled(),
        Some(iced_accessibility::accesskit::Toggled::True)
    );
    assert!(tree.root()[0].node().is_disabled());
}

#[test]
fn unchecked_checkbox_reports_false_toggled_state() {
    let mut checkbox = checkbox(false);
    let mut state = Tree::new::<(), Theme, Renderer>(&checkbox);

    <_ as Widget<(), Theme, Renderer>>::diff(&mut checkbox, &mut state);

    let tree = <_ as Widget<(), Theme, Renderer>>::a11y_nodes(
        &checkbox,
        Layout::new(Size::new(24.0, 24.0)),
        &state,
        mouse::Cursor::Unavailable,
    );

    assert_eq!(
        tree.root()[0].node().toggled(),
        Some(iced_accessibility::accesskit::Toggled::False)
    );
    assert!(tree.root()[0].node().is_disabled());
}

#[test]
fn stack_does_not_hide_accessible_children() {
    let mut stack = stack([text("Stacked")]);
    let mut state = Tree::new::<(), Theme, Renderer>(&stack);

    <_ as Widget<(), Theme, Renderer>>::diff(&mut stack, &mut state);

    let tree = <_ as Widget<(), Theme, Renderer>>::a11y_nodes(
        &stack,
        Layout::new(Size::new(120.0, 32.0)),
        &state,
        mouse::Cursor::Unavailable,
    );

    assert_eq!(tree.root().len(), 1);
}

#[test]
fn container_passes_accessible_children_through() {
    let mut container = container(text("Contained"));
    let mut state = Tree::new::<(), Theme, Renderer>(&container);

    <_ as Widget<(), Theme, Renderer>>::diff(&mut container, &mut state);

    let tree = <_ as Widget<(), Theme, Renderer>>::a11y_nodes(
        &container,
        Layout::new(Size::new(120.0, 32.0)),
        &state,
        mouse::Cursor::Unavailable,
    );

    assert_eq!(tree.root().len(), 1);
    assert!(tree.children().is_empty());
}
