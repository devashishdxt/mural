use crate::block::{Block, ErasedBlock};

#[derive(Default)]
pub(crate) struct Region {
    entries: Vec<BlockEntry>,
}

struct BlockEntry {
    id: Option<String>,
    block: CachedBlock,
}

struct CachedBlock {
    block: Box<dyn ErasedBlock>,
    dirty: bool,
    cached_width: Option<usize>,
    cached_lines: Vec<String>,
}

impl Region {
    pub(crate) fn push<BlockType>(&mut self, block: BlockType)
    where
        BlockType: Block + 'static,
    {
        self.entries.push(BlockEntry {
            id: None,
            block: CachedBlock::new(block),
        });
    }

    pub(crate) fn insert<Id, BlockType>(&mut self, id: Id, block: BlockType)
    where
        Id: Into<String>,
        BlockType: Block + 'static,
    {
        let id = id.into();
        if let Some(entry) = self.entry_mut(&id) {
            entry.block = CachedBlock::new(block);
            return;
        }

        self.entries.push(BlockEntry {
            id: Some(id),
            block: CachedBlock::new(block),
        });
    }

    pub(crate) fn get<BlockType, Id>(&self, id: Id) -> Option<&BlockType>
    where
        BlockType: Block + 'static,
        Id: AsRef<str>,
    {
        self.entry(id.as_ref())?.block.get::<BlockType>()
    }

    pub(crate) fn get_mut<BlockType, Id>(&mut self, id: Id) -> Option<&mut BlockType>
    where
        BlockType: Block + 'static,
        Id: AsRef<str>,
    {
        self.entry_mut(id.as_ref())?.block.get_mut::<BlockType>()
    }

    pub(crate) fn remove<Id>(&mut self, id: Id) -> bool
    where
        Id: AsRef<str>,
    {
        let id = id.as_ref();
        let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.id.as_deref() == Some(id))
        else {
            return false;
        };

        self.entries.remove(index);
        true
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
    }

    pub(crate) fn render_lines(&mut self, width: usize) -> Vec<String> {
        let mut lines = Vec::new();
        for entry in &mut self.entries {
            lines.extend(entry.block.render(width).iter().cloned());
        }
        lines
    }

    pub(crate) fn mark_all_dirty(&mut self) {
        for entry in &mut self.entries {
            entry.block.mark_dirty();
        }
    }

    pub(crate) fn mark_all_clean(&mut self) {
        for entry in &mut self.entries {
            entry.block.mark_clean();
        }
    }

    fn entry(&self, id: &str) -> Option<&BlockEntry> {
        self.entries
            .iter()
            .find(|entry| entry.id.as_deref() == Some(id))
    }

    fn entry_mut(&mut self, id: &str) -> Option<&mut BlockEntry> {
        self.entries
            .iter_mut()
            .find(|entry| entry.id.as_deref() == Some(id))
    }
}

impl CachedBlock {
    fn new<BlockType>(block: BlockType) -> Self
    where
        BlockType: Block + 'static,
    {
        Self {
            block: Box::new(block),
            dirty: true,
            cached_width: None,
            cached_lines: Vec::new(),
        }
    }

    fn get<BlockType>(&self) -> Option<&BlockType>
    where
        BlockType: Block + 'static,
    {
        self.block.as_any().downcast_ref()
    }

    fn get_mut<BlockType>(&mut self) -> Option<&mut BlockType>
    where
        BlockType: Block + 'static,
    {
        if !self.block.as_any().is::<BlockType>() {
            return None;
        }

        self.mark_dirty();
        self.block.as_any_mut().downcast_mut()
    }

    fn render(&mut self, width: usize) -> &[String] {
        if self.should_render(width) {
            let rendered_lines = self.block.render(width);
            debug_assert!(
                rendered_lines
                    .iter()
                    .all(|line| !line.contains('\n') && !line.contains('\r'))
            );
            self.cached_lines = rendered_lines
                .into_iter()
                .map(|line| line.into_owned())
                .collect();
            self.cached_width = Some(width);
        }

        &self.cached_lines
    }

    fn should_render(&self, width: usize) -> bool {
        self.dirty || self.cached_width != Some(width) || self.block.render_every_frame()
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn mark_clean(&mut self) {
        if self.cached_width.is_some() {
            self.dirty = false;
        }
    }
}
