use std::{cell::Cell, rc::Rc};

use brisk::{
    Line, Render, Size, Terminal, Text,
    backend::fake::{FakeBackend, Operation},
};

struct WidthEcho;

impl Render for WidthEcho {
    fn render(&self, width: u16) -> Text {
        Text::from_plain(format!("width={width}")).unwrap()
    }
}

#[derive(Debug)]
struct CountingBlock {
    content: String,
    render_count: Rc<Cell<usize>>,
}

impl CountingBlock {
    fn new(content: impl Into<String>, render_count: Rc<Cell<usize>>) -> Self {
        Self {
            content: content.into(),
            render_count,
        }
    }
}

impl Render for CountingBlock {
    fn render(&self, _width: u16) -> Text {
        self.render_count.set(self.render_count.get() + 1);
        Text::from_plain(&self.content).unwrap()
    }
}

struct WidthRecorder {
    seen_width: Rc<Cell<u16>>,
}

impl Render for WidthRecorder {
    fn render(&self, width: u16) -> Text {
        self.seen_width.set(width);
        Text::empty()
    }
}

#[test]
fn resize_defers_terminal_io_and_uses_new_safe_printable_width_on_render() {
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal.push_live(WidthEcho).unwrap();

    terminal.resize(Size::new(9, 24)).unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[Operation::QuerySize, Operation::HideCursor]
    );

    terminal.render().unwrap();

    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::MoveToOrigin,
            Operation::Clear,
            Operation::PurgeScrollback,
            Operation::Print(Line::from_plain("width=8").unwrap()),
            Operation::Flush,
        ]
    );
}

#[test]
fn width_change_rerenders_clean_blocks_and_forces_full_rewrite() {
    let render_count = Rc::new(Cell::new(0));
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .push_live(CountingBlock::new("unchanged", Rc::clone(&render_count)))
        .unwrap();
    terminal.render().unwrap();
    let after_first_render = terminal.backend().operations().len();

    terminal.resize(Size::new(10, 24)).unwrap();
    terminal.render().unwrap();

    assert_eq!(render_count.get(), 2);
    assert_eq!(
        &terminal.backend().operations()[after_first_render..],
        &[
            Operation::MoveToOrigin,
            Operation::Clear,
            Operation::PurgeScrollback,
            Operation::Print(Line::from_plain("unchanged").unwrap()),
            Operation::Flush,
        ]
    );
}

#[test]
fn height_only_change_full_rewrites_without_rerendering_clean_blocks() {
    let render_count = Rc::new(Cell::new(0));
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .push_live(CountingBlock::new("cached", Rc::clone(&render_count)))
        .unwrap();
    terminal.render().unwrap();
    let after_first_render = terminal.backend().operations().len();

    terminal.resize(Size::new(80, 10)).unwrap();
    terminal.render().unwrap();

    assert_eq!(render_count.get(), 1);
    assert_eq!(
        &terminal.backend().operations()[after_first_render..],
        &[
            Operation::MoveToOrigin,
            Operation::Clear,
            Operation::PurgeScrollback,
            Operation::Print(Line::from_plain("cached").unwrap()),
            Operation::Flush,
        ]
    );
}

#[test]
fn zero_width_resize_uses_zero_safe_printable_width() {
    let seen_width = Rc::new(Cell::new(u16::MAX));
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .push_live(WidthRecorder {
            seen_width: Rc::clone(&seen_width),
        })
        .unwrap();

    terminal.resize(Size::new(0, 24)).unwrap();
    terminal.render().unwrap();

    assert_eq!(seen_width.get(), 0);
    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::MoveToOrigin,
            Operation::Clear,
            Operation::PurgeScrollback,
            Operation::Flush,
        ]
    );
}
