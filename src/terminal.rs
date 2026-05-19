use crate::{Backend, Line, Render, Size, StdoutBackend};
use std::io;

pub struct Terminal<B> {
    backend: B,
    size: Size,
    live_blocks: Vec<Box<dyn Render>>,
    pinned_blocks: Vec<Box<dyn Render>>,
    finished: bool,
}

impl Terminal<StdoutBackend<io::Stdout>> {
    pub fn stdout() -> io::Result<Self> {
        Self::new(StdoutBackend::stdout())
    }
}

impl<B: Backend> Terminal<B> {
    pub fn new(mut backend: B) -> io::Result<Self> {
        let size = backend.size()?;
        backend.hide_cursor()?;
        Ok(Self {
            backend,
            size,
            live_blocks: Vec::new(),
            pinned_blocks: Vec::new(),
            finished: false,
        })
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn append_live(&mut self, block: impl Render + 'static) -> io::Result<()> {
        self.ensure_unfinished()?;
        self.live_blocks.push(Box::new(block));
        Ok(())
    }

    pub fn append_pinned(&mut self, block: impl Render + 'static) -> io::Result<()> {
        self.ensure_unfinished()?;
        self.pinned_blocks.push(Box::new(block));
        Ok(())
    }

    pub fn render(&mut self) -> io::Result<()> {
        self.ensure_unfinished()?;
        let lines = self.rendered_lines();
        self.print_lines(&lines)?;
        self.backend.flush()
    }

    pub fn finish(&mut self) -> io::Result<()> {
        if self.finished {
            self.backend.show_cursor()?;
            return self.backend.flush();
        }

        let lines = self.rendered_live_lines();
        self.backend.clear()?;
        self.print_lines(&lines)?;
        if !lines.is_empty() {
            self.backend.newline()?;
        }
        self.backend.move_to_column(0)?;
        self.backend.show_cursor()?;
        self.backend.flush()?;
        self.finished = true;
        Ok(())
    }

    fn ensure_unfinished(&self) -> io::Result<()> {
        if self.finished {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "terminal has already finished",
            ))
        } else {
            Ok(())
        }
    }

    fn print_lines(&mut self, lines: &[Line]) -> io::Result<()> {
        for (index, line) in lines.iter().enumerate() {
            if index > 0 {
                self.backend.newline()?;
            }
            self.backend.print(line)?;
        }
        Ok(())
    }

    fn rendered_lines(&self) -> Vec<Line> {
        self.render_blocks(self.live_blocks.iter().chain(self.pinned_blocks.iter()))
    }

    fn rendered_live_lines(&self) -> Vec<Line> {
        self.render_blocks(self.live_blocks.iter())
    }

    fn render_blocks<'a>(&self, blocks: impl Iterator<Item = &'a Box<dyn Render>>) -> Vec<Line> {
        let safe_width = self.size.width().saturating_sub(1);
        blocks
            .flat_map(|block| {
                block
                    .render(safe_width)
                    .into_wrapped(usize::from(safe_width))
                    .lines()
                    .to_vec()
            })
            .collect()
    }
}
