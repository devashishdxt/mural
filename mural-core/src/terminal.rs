use thiserror::Error;

use crate::{
    backend::Backend,
    block::{Block, RenderContext},
    color_scheme::ColorScheme,
    frame::Frame,
    region::Region,
    renderer::Renderer,
};

#[derive(Debug, Error)]
pub enum Error<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[error("invalid terminal size: width and height must be greater than zero")]
    InvalidTerminalSize,

    #[error("invalid cursor position: row/column must be within terminal size")]
    InvalidCursorPosition,

    #[error(transparent)]
    Backend(#[from] E),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub height: usize,
    pub width: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPosition {
    pub row: usize,
    pub column: usize,
}

pub struct Terminal<B> {
    backend: B,
    live_region: Region,
    pinned_region: Region,
    size: TerminalSize,
    color_scheme: ColorScheme,
    needs_full_redraw: bool,
    committed_frame: Frame,
    sentinel_row: usize,
}

impl<B> Terminal<B>
where
    B: Backend,
{
    pub fn new(
        mut backend: B,
        size: TerminalSize,
        mut position: CursorPosition,
        color_scheme: ColorScheme,
    ) -> Result<Self, Error<B::Error>> {
        validate_size(size)?;
        validate_position(size, position)?;

        backend.hide_cursor()?;
        position = normalize_initial_position(&mut backend, size, position)?;
        backend.flush()?;

        Ok(Self {
            backend,
            live_region: Default::default(),
            pinned_region: Default::default(),
            size,
            color_scheme,
            needs_full_redraw: false,
            committed_frame: Frame::default(),
            sentinel_row: position.row,
        })
    }

    pub fn push_live(&mut self, block: impl Block + 'static) {
        self.live_region.push(block);
    }

    pub fn push_pinned(&mut self, block: impl Block + 'static) {
        self.pinned_region.push(block);
    }

    pub fn insert_live(&mut self, id: impl Into<String>, block: impl Block + 'static) {
        self.live_region.insert(id, block);
    }

    pub fn insert_pinned(&mut self, id: impl Into<String>, block: impl Block + 'static) {
        self.pinned_region.insert(id, block);
    }

    pub fn get_live<T>(&self, id: impl AsRef<str>) -> Option<&T>
    where
        T: Block + 'static,
    {
        self.live_region.get(id)
    }

    pub fn get_pinned<T>(&self, id: impl AsRef<str>) -> Option<&T>
    where
        T: Block + 'static,
    {
        self.pinned_region.get(id)
    }

    pub fn get_live_mut<T>(&mut self, id: impl AsRef<str>) -> Option<&mut T>
    where
        T: Block + 'static,
    {
        self.live_region.get_mut(id)
    }

    pub fn get_pinned_mut<T>(&mut self, id: impl AsRef<str>) -> Option<&mut T>
    where
        T: Block + 'static,
    {
        self.pinned_region.get_mut(id)
    }

    pub fn remove_live(&mut self, id: impl AsRef<str>) {
        self.live_region.remove(id);
    }

    pub fn remove_pinned(&mut self, id: impl AsRef<str>) {
        self.pinned_region.remove(id);
    }

    pub fn clear_live(&mut self) {
        self.live_region.clear();
    }

    pub fn clear_pinned(&mut self) {
        self.pinned_region.clear();
    }

    pub fn clear_all(&mut self) {
        self.clear_live();
        self.clear_pinned();
    }

    pub fn resize(&mut self, size: TerminalSize) -> Result<(), Error<B::Error>> {
        validate_size(size)?;

        self.size = size;
        self.needs_full_redraw = true;

        Ok(())
    }

    pub fn set_color_scheme(&mut self, color_scheme: ColorScheme) {
        if self.color_scheme == color_scheme {
            return;
        }

        self.color_scheme = color_scheme;
        self.needs_full_redraw = true;
    }

    pub fn force_full_redraw(&mut self) {
        self.needs_full_redraw = true;
    }

    pub fn render(&mut self) -> Result<(), Error<B::Error>> {
        self.render_frame(true)
    }

    pub fn finish(&mut self) -> Result<(), Error<B::Error>> {
        self.render_frame(false)?;
        self.backend.show_cursor()?;
        self.backend.flush()?;

        Ok(())
    }

    fn render_frame(&mut self, include_pinned: bool) -> Result<(), Error<B::Error>> {
        let context = RenderContext {
            width: self.size.width.saturating_sub(1),
            color_scheme: self.color_scheme,
        };
        let mut new_frame = self.live_region.render(&context);

        if include_pinned {
            new_frame.extend(self.pinned_region.render(&context));
        }

        self.sentinel_row = match Renderer::new(
            &self.committed_frame,
            &new_frame,
            self.size.height,
            self.sentinel_row,
        )
        .render(&mut self.backend, self.needs_full_redraw)
        {
            Ok(sentinel_row) => sentinel_row,
            Err(err) => {
                self.needs_full_redraw = true;
                return Err(err.into());
            }
        };

        self.committed_frame = new_frame;

        self.live_region.mark_all_clean();
        if include_pinned {
            self.pinned_region.mark_all_clean();
        }

        self.needs_full_redraw = false;

        Ok(())
    }
}

fn validate_size<E>(size: TerminalSize) -> Result<(), Error<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    if size.width == 0 || size.height == 0 {
        Err(Error::InvalidTerminalSize)
    } else {
        Ok(())
    }
}

fn validate_position<E>(size: TerminalSize, position: CursorPosition) -> Result<(), Error<E>>
where
    E: std::error::Error + Send + Sync + 'static,
{
    if position.row >= size.height || position.column >= size.width {
        Err(Error::InvalidCursorPosition)
    } else {
        Ok(())
    }
}

fn normalize_initial_position<B: Backend>(
    backend: &mut B,
    size: TerminalSize,
    position: CursorPosition,
) -> Result<CursorPosition, Error<B::Error>> {
    if position.column == 0 {
        return Ok(position);
    }

    backend.newline()?;
    Ok(CursorPosition {
        row: position.row.saturating_add(1).min(size.height - 1),
        column: 0,
    })
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use std::{borrow::Cow, cell::Cell, io, rc::Rc};

    use super::{CursorPosition, Error, Terminal, TerminalSize};
    use crate::{
        ColorScheme,
        backend::Backend,
        block::{Block, RenderContext},
    };

    fn context(width: usize) -> RenderContext {
        RenderContext {
            width,
            color_scheme: ColorScheme::Dark,
        }
    }

    struct CountingBlock {
        renders: Rc<Cell<usize>>,
    }

    impl Block for CountingBlock {
        fn render(&self, context: &RenderContext) -> Vec<Cow<'_, str>> {
            self.renders.set(self.renders.get() + 1);
            vec![Cow::Owned(format!(
                "{}:{:?}",
                context.width(),
                context.color_scheme()
            ))]
        }
    }

    #[derive(Default)]
    struct MockBackend {
        calls: Vec<String>,
        fail_on: Option<&'static str>,
    }

    impl MockBackend {
        fn call(&mut self, name: &str) -> io::Result<()> {
            self.calls.push(name.to_owned());
            if self.fail_on == Some(name) {
                self.fail_on = None;
                Err(io::Error::other(name.to_owned()))
            } else {
                Ok(())
            }
        }
    }

    impl Backend for MockBackend {
        type Error = io::Error;

        fn hide_cursor(&mut self) -> Result<(), Self::Error> {
            self.call("hide_cursor")
        }
        fn show_cursor(&mut self) -> Result<(), Self::Error> {
            self.call("show_cursor")
        }
        fn move_up(&mut self, n: usize) -> Result<(), Self::Error> {
            self.call(&format!("move_up:{n}"))
        }
        fn move_down(&mut self, n: usize) -> Result<(), Self::Error> {
            self.call(&format!("move_down:{n}"))
        }
        fn carriage_return(&mut self) -> Result<(), Self::Error> {
            self.call("carriage_return")
        }
        fn newline(&mut self) -> Result<(), Self::Error> {
            self.call("newline")
        }
        fn scroll_up(&mut self, n: usize) -> Result<(), Self::Error> {
            self.call(&format!("scroll_up:{n}"))
        }
        fn insert_lines(&mut self, n: usize) -> Result<(), Self::Error> {
            self.call(&format!("insert_lines:{n}"))
        }
        fn delete_lines(&mut self, n: usize) -> Result<(), Self::Error> {
            self.call(&format!("delete_lines:{n}"))
        }
        fn clear_line(&mut self) -> Result<(), Self::Error> {
            self.call("clear_line")
        }
        fn write_str(&mut self, text: &str) -> Result<(), Self::Error> {
            self.calls.push(format!("write:{text}"));
            if self.fail_on == Some("write") {
                self.fail_on = None;
                Err(io::Error::other("write"))
            } else {
                Ok(())
            }
        }
        fn clear_screen(&mut self) -> Result<(), Self::Error> {
            self.call("clear_screen")
        }
        fn purge_scrollback(&mut self) -> Result<(), Self::Error> {
            self.call("purge_scrollback")
        }
        fn move_to_top_left(&mut self) -> Result<(), Self::Error> {
            self.call("move_to_top_left")
        }
        fn flush(&mut self) -> Result<(), Self::Error> {
            self.call("flush")
        }
    }

    fn size() -> TerminalSize {
        TerminalSize {
            height: 5,
            width: 20,
        }
    }

    fn position(row: usize, column: usize) -> CursorPosition {
        CursorPosition { row, column }
    }

    #[test]
    fn constructor_validates_size_and_position() {
        assert!(matches!(
            Terminal::new(
                MockBackend::default(),
                TerminalSize {
                    height: 0,
                    width: 20,
                },
                position(0, 0),
                ColorScheme::Dark,
            ),
            Err(Error::InvalidTerminalSize)
        ));
        assert!(matches!(
            Terminal::new(
                MockBackend::default(),
                size(),
                position(5, 0),
                ColorScheme::Dark
            ),
            Err(Error::InvalidCursorPosition)
        ));
        assert!(matches!(
            Terminal::new(
                MockBackend::default(),
                size(),
                position(0, 20),
                ColorScheme::Dark
            ),
            Err(Error::InvalidCursorPosition)
        ));
    }

    #[test]
    fn constructor_hides_cursor_and_normalizes_nonzero_column() {
        let at_start = Terminal::new(
            MockBackend::default(),
            size(),
            position(2, 0),
            ColorScheme::Dark,
        )
        .unwrap();
        assert_eq!(at_start.sentinel_row, 2);
        assert_eq!(at_start.backend.calls, ["hide_cursor", "flush"]);

        let normalized = Terminal::new(
            MockBackend::default(),
            size(),
            position(4, 3),
            ColorScheme::Dark,
        )
        .unwrap();
        assert_eq!(normalized.sentinel_row, 4);
        assert_eq!(
            normalized.backend.calls,
            ["hide_cursor", "newline", "flush"]
        );
    }

    #[test]
    fn constructor_propagates_backend_errors() {
        let backend = MockBackend {
            fail_on: Some("hide_cursor"),
            ..Default::default()
        };
        assert!(matches!(
            Terminal::new(backend, size(), position(0, 0), ColorScheme::Dark),
            Err(Error::Backend(_))
        ));

        let backend = MockBackend {
            fail_on: Some("newline"),
            ..Default::default()
        };
        assert!(matches!(
            Terminal::new(backend, size(), position(0, 1), ColorScheme::Dark),
            Err(Error::Backend(_))
        ));
    }

    #[test]
    fn region_api_manages_live_and_pinned_blocks() {
        let mut terminal = Terminal::new(
            MockBackend::default(),
            size(),
            position(0, 0),
            ColorScheme::Dark,
        )
        .unwrap();
        terminal.push_live("anonymous live");
        terminal.push_pinned("anonymous pinned");
        terminal.insert_live("live", String::from("live value"));
        terminal.insert_pinned("pinned", String::from("pinned value"));

        assert_eq!(terminal.get_live::<String>("live").unwrap(), "live value");
        assert_eq!(
            terminal.get_pinned::<String>("pinned").unwrap(),
            "pinned value"
        );
        terminal.get_live_mut::<String>("live").unwrap().push('!');
        terminal
            .get_pinned_mut::<String>("pinned")
            .unwrap()
            .push('!');

        terminal.remove_live("live");
        terminal.remove_pinned("pinned");
        assert!(terminal.get_live::<String>("live").is_none());
        assert!(terminal.get_pinned::<String>("pinned").is_none());

        terminal.clear_live();
        terminal.clear_pinned();
        assert_eq!(terminal.live_region.render(&context(10)).len(), 0);
        assert_eq!(terminal.pinned_region.render(&context(10)).len(), 0);

        terminal.push_live("live");
        terminal.push_pinned("pinned");
        terminal.clear_all();
        assert_eq!(terminal.live_region.render(&context(10)).len(), 0);
        assert_eq!(terminal.pinned_region.render(&context(10)).len(), 0);
    }

    #[test]
    fn render_includes_pinned_blocks_but_finish_removes_them() {
        let mut terminal = Terminal::new(
            MockBackend::default(),
            size(),
            position(0, 0),
            ColorScheme::Dark,
        )
        .unwrap();
        terminal.push_live("live");
        terminal.push_pinned("pinned");
        terminal.backend.calls.clear();

        terminal.render().unwrap();
        assert!(terminal.backend.calls.contains(&"write:live".to_owned()));
        assert!(terminal.backend.calls.contains(&"write:pinned".to_owned()));
        assert_eq!(terminal.committed_frame.len(), 2);

        terminal.backend.calls.clear();
        terminal.finish().unwrap();
        assert_eq!(terminal.committed_frame.len(), 1);
        assert!(
            terminal
                .backend
                .calls
                .contains(&"delete_lines:1".to_owned())
        );
        assert!(
            terminal
                .backend
                .calls
                .ends_with(&["show_cursor".to_owned(), "flush".to_owned()])
        );
    }

    #[test]
    fn resize_rerenders_blocks_only_when_width_changes() {
        let renders = Rc::new(Cell::new(0));
        let mut terminal = Terminal::new(
            MockBackend::default(),
            size(),
            position(0, 0),
            ColorScheme::Dark,
        )
        .unwrap();
        terminal.push_live(CountingBlock {
            renders: Rc::clone(&renders),
        });

        terminal.render().unwrap();
        assert_eq!(renders.get(), 1);

        terminal
            .resize(TerminalSize {
                height: 6,
                width: 20,
            })
            .unwrap();
        terminal.render().unwrap();
        assert_eq!(renders.get(), 1);

        terminal
            .resize(TerminalSize {
                height: 6,
                width: 10,
            })
            .unwrap();
        terminal.render().unwrap();
        assert_eq!(renders.get(), 2);
    }

    #[test]
    fn color_scheme_changes_rerender_blocks_and_reset_the_screen() {
        let renders = Rc::new(Cell::new(0));
        let mut terminal = Terminal::new(
            MockBackend::default(),
            size(),
            position(0, 0),
            ColorScheme::Dark,
        )
        .unwrap();
        terminal.push_live(CountingBlock {
            renders: Rc::clone(&renders),
        });

        terminal.render().unwrap();
        assert_eq!(&terminal.committed_frame[0], "19:Dark");
        assert_eq!(renders.get(), 1);

        terminal.backend.calls.clear();
        terminal.set_color_scheme(ColorScheme::Dark);
        terminal.render().unwrap();
        assert_eq!(renders.get(), 1);
        assert!(!terminal.backend.calls.contains(&"clear_screen".to_owned()));

        terminal.backend.calls.clear();
        terminal.set_color_scheme(ColorScheme::Light);
        terminal.render().unwrap();
        assert_eq!(&terminal.committed_frame[0], "19:Light");
        assert_eq!(renders.get(), 2);
        assert_eq!(terminal.backend.calls[0], "clear_screen");
    }

    #[test]
    fn resize_and_force_redraw_reset_the_screen() {
        let mut terminal = Terminal::new(
            MockBackend::default(),
            size(),
            position(0, 0),
            ColorScheme::Dark,
        )
        .unwrap();
        terminal.push_live("content");
        terminal.render().unwrap();

        assert!(matches!(
            terminal.resize(TerminalSize {
                height: 5,
                width: 0,
            }),
            Err(Error::InvalidTerminalSize)
        ));

        terminal
            .resize(TerminalSize {
                height: 6,
                width: 20,
            })
            .unwrap();
        terminal.backend.calls.clear();
        terminal.render().unwrap();
        assert_eq!(terminal.backend.calls[0], "clear_screen");

        terminal
            .resize(TerminalSize {
                height: 6,
                width: 10,
            })
            .unwrap();
        terminal.render().unwrap();

        terminal.force_full_redraw();
        terminal.backend.calls.clear();
        terminal.render().unwrap();
        assert_eq!(terminal.backend.calls[0], "clear_screen");
    }

    #[test]
    fn failed_render_is_retried_as_a_full_redraw() {
        let mut terminal = Terminal::new(
            MockBackend::default(),
            size(),
            position(0, 0),
            ColorScheme::Dark,
        )
        .unwrap();
        terminal.push_live("content");
        terminal.backend.fail_on = Some("write");

        assert!(matches!(terminal.render(), Err(Error::Backend(_))));
        assert!(terminal.needs_full_redraw);
        assert_eq!(terminal.committed_frame.len(), 0);

        terminal.backend.calls.clear();
        terminal.render().unwrap();
        assert_eq!(terminal.backend.calls[0], "clear_screen");
        assert!(!terminal.needs_full_redraw);
        assert_eq!(terminal.committed_frame.len(), 1);
    }

    #[test]
    fn finish_propagates_show_cursor_errors() {
        let mut terminal = Terminal::new(
            MockBackend::default(),
            size(),
            position(0, 0),
            ColorScheme::Dark,
        )
        .unwrap();
        terminal.backend.fail_on = Some("show_cursor");

        assert!(matches!(terminal.finish(), Err(Error::Backend(_))));
        assert_eq!(terminal.backend.calls.last().unwrap(), "show_cursor");
    }
}
