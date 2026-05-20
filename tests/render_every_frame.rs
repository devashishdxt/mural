use std::{cell::Cell, rc::Rc};

use brisk::{
    Line, Render, Size, Terminal, Text,
    backend::fake::{FakeBackend, Operation},
};

struct AlwaysRenderBlock {
    content: &'static str,
    render_count: Rc<Cell<usize>>,
}

impl AlwaysRenderBlock {
    fn new(content: &'static str, render_count: Rc<Cell<usize>>) -> Self {
        Self {
            content,
            render_count,
        }
    }
}

impl Render for AlwaysRenderBlock {
    fn render(&self, _width: u16) -> Text {
        self.render_count.set(self.render_count.get() + 1);
        Text::from_plain(self.content).unwrap()
    }

    fn render_every_frame(&self) -> bool {
        true
    }
}

#[test]
fn always_render_blocks_rerender_on_clean_frames_without_unnecessary_terminal_output() {
    let render_count = Rc::new(Cell::new(0));
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .push_live(AlwaysRenderBlock::new("same", Rc::clone(&render_count)))
        .unwrap();

    terminal.render().unwrap();
    let after_first_render = terminal.backend().operations().len();
    terminal.render().unwrap();

    assert_eq!(render_count.get(), 2);
    assert_eq!(
        &terminal.backend().operations()[after_first_render..],
        &[Operation::Flush]
    );
    assert_eq!(
        terminal.backend().operations(),
        &[
            Operation::QuerySize,
            Operation::HideCursor,
            Operation::Print(Line::from_plain("same").unwrap()),
            Operation::Flush,
            Operation::Flush,
        ]
    );
}

#[test]
fn render_every_frame_is_a_dynamic_hint() {
    struct DynamicRenderBlock {
        render_count: Rc<Cell<usize>>,
        render_every_frame: Rc<Cell<bool>>,
    }

    impl Render for DynamicRenderBlock {
        fn render(&self, _width: u16) -> Text {
            self.render_count.set(self.render_count.get() + 1);
            Text::from_plain("dynamic").unwrap()
        }

        fn render_every_frame(&self) -> bool {
            self.render_every_frame.get()
        }
    }

    let render_count = Rc::new(Cell::new(0));
    let render_every_frame = Rc::new(Cell::new(true));
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .push_live(DynamicRenderBlock {
            render_count: Rc::clone(&render_count),
            render_every_frame: Rc::clone(&render_every_frame),
        })
        .unwrap();

    terminal.render().unwrap();
    terminal.render().unwrap();
    render_every_frame.set(false);
    terminal.render().unwrap();

    assert_eq!(render_count.get(), 2);
}

#[test]
fn always_render_blocks_report_effective_dirtiness_after_successful_render() {
    let render_count = Rc::new(Cell::new(0));
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .insert_live(
            "clock",
            AlwaysRenderBlock::new("tick", Rc::clone(&render_count)),
        )
        .unwrap();

    assert_eq!(terminal.is_block_dirty("clock"), Ok(true));
    terminal.render().unwrap();

    assert_eq!(terminal.is_block_dirty("clock"), Ok(true));
}

#[test]
fn finish_rerenders_live_always_render_blocks_and_skips_pinned_blocks() {
    let live_render_count = Rc::new(Cell::new(0));
    let pinned_render_count = Rc::new(Cell::new(0));
    let mut terminal = Terminal::new(FakeBackend::new(Size::new(80, 24))).unwrap();
    terminal
        .push_live(AlwaysRenderBlock::new(
            "transcript",
            Rc::clone(&live_render_count),
        ))
        .unwrap();
    terminal
        .push_pinned(AlwaysRenderBlock::new(
            "status",
            Rc::clone(&pinned_render_count),
        ))
        .unwrap();

    terminal.render().unwrap();
    terminal.finish().unwrap();

    assert_eq!(live_render_count.get(), 2);
    assert_eq!(pinned_render_count.get(), 1);
}
