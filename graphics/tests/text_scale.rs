//! Regression coverage for global text scaling.

use iced_graphics::core::{
    self, Font, Pixels, Size,
    alignment::Vertical,
    text::{Alignment, Ellipsis, LineHeight, Paragraph as _, Shaping, Wrapping},
};

struct RestoreTextScale(f32);

impl Drop for RestoreTextScale {
    fn drop(&mut self) {
        iced_graphics::text::set_text_scale(self.0);
    }
}

#[test]
fn global_text_scale_applies_to_paragraphs_and_cached_text() {
    let original_scale = iced_graphics::text::text_scale();
    let _restore = RestoreTextScale(original_scale);

    iced_graphics::text::set_text_scale(2.0);

    let paragraph = iced_graphics::text::Paragraph::with_text(core::text::Text {
        content: "scaled paragraph",
        bounds: Size::new(300.0, 100.0),
        size: Pixels(16.0),
        line_height: LineHeight::Relative(1.25),
        font: Font::default(),
        align_x: Alignment::Left,
        align_y: Vertical::Top,
        shaping: Shaping::Basic,
        wrapping: Wrapping::None,
        ellipsis: Ellipsis::None,
        hint_factor: None,
    });

    assert_eq!(paragraph.size(), Pixels(32.0));
    assert_eq!(paragraph.line_height(), LineHeight::Absolute(Pixels(40.0)));

    let mut font_system = iced_graphics::text::font_system()
        .write()
        .expect("font system lock");
    let mut cache = iced_graphics::text::Cache::new();
    let (_, entry) = cache.allocate(
        font_system.raw(),
        iced_graphics::text::cache::Key {
            content: "scaled cached text",
            size: 16.0,
            line_height: 20.0,
            font: Font::default(),
            bounds: Size::new(300.0, 100.0),
            shaping: Shaping::Basic,
            align_x: Alignment::Left,
            wrapping: Wrapping::None,
            ellipsis: Ellipsis::None,
        },
    );

    assert_eq!(entry.buffer.metrics().font_size, 32.0);
    assert_eq!(entry.buffer.metrics().line_height, 40.0);
    drop(font_system);

    let mut editor =
        <iced_graphics::text::Editor as core::text::editor::Editor>::with_text("scaled editor");
    let mut parser = core::text::parser::PlainText;
    core::text::editor::Editor::update(
        &mut editor,
        Size::new(300.0, 100.0),
        Font::default(),
        Pixels(16.0),
        LineHeight::Absolute(Pixels(20.0)),
        Wrapping::Word,
        Alignment::Left,
        None,
        &mut parser,
    );

    assert_eq!(editor.buffer().metrics().font_size, 32.0);
    assert_eq!(editor.buffer().metrics().line_height, 40.0);
}
