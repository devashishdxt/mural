use crate::{Backend, Render, Size, StdoutBackend};
use std::io;

pub struct Terminal<B> {
    backend: B,
    size: Size,
    live_blocks: Vec<Box<dyn Render>>,
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
        })
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }

    pub fn backend_mut(&mut self) -> &mut B {
        &mut self.backend
    }

    pub fn append_live(&mut self, block: impl Render + 'static) -> io::Result<()> {
        self.live_blocks.push(Box::new(block));
        Ok(())
    }

    pub fn render(&mut self) -> io::Result<()> {
        let lines = self.rendered_lines();
        for (index, line) in lines.iter().enumerate() {
            if index > 0 {
                self.backend.newline()?;
            }
            self.backend.print(line)?;
        }
        self.backend.flush()
    }

    fn rendered_lines(&self) -> Vec<String> {
        let safe_width = self.size.width().saturating_sub(1);
        self.live_blocks
            .iter()
            .flat_map(|block| block.render(safe_width))
            .collect()
    }
}
