use crate::block::{Block, ErasedBlock};

struct CachedBlock {
    block: Box<dyn ErasedBlock>,
    dirty: bool,
    width: Option<usize>,
    lines: Vec<String>,
}

impl CachedBlock {
    fn new(block: impl Block + 'static) -> Self {
        Self {
            block: Box::new(block),
            dirty: true,
            width: None,
            lines: Vec::new(),
        }
    }

    fn get<B>(&self) -> Option<&B>
    where
        B: Block + 'static,
    {
        self.block.as_any().downcast_ref()
    }

    fn get_mut<B>(&mut self) -> Option<&mut B>
    where
        B: Block + 'static,
    {
        if !self.block.as_any().is::<B>() {
            return None;
        }

        self.mark_dirty();
        self.block.as_any_mut().downcast_mut()
    }

    fn render(&mut self, width: usize) -> &[String] {
        if self.should_render(width) {
            self.refresh(width);
        }

        &self.lines
    }

    fn refresh(&mut self, width: usize) {
        let rendered_lines = self.block.render(width);

        debug_assert!(
            rendered_lines
                .iter()
                .all(|line| !line.contains('\n') && !line.contains('\r'))
        );

        self.lines = rendered_lines
            .into_iter()
            .map(|line| line.into_owned())
            .collect();
        self.width = Some(width);
    }

    fn should_render(&self, width: usize) -> bool {
        self.dirty || self.width != Some(width) || self.block.render_every_frame()
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn mark_clean(&mut self) {
        if self.width.is_some() {
            self.dirty = false;
        }
    }
}

struct BlockEntry {
    id: Option<String>,
    block: CachedBlock,
}

impl BlockEntry {
    fn anonymous(block: impl Block + 'static) -> Self {
        Self {
            id: None,
            block: CachedBlock::new(block),
        }
    }

    fn identified(id: impl Into<String>, block: impl Block + 'static) -> Self {
        Self {
            id: Some(id.into()),
            block: CachedBlock::new(block),
        }
    }

    fn match_id(&self, id: impl AsRef<str>) -> bool {
        self.id.as_deref() == Some(id.as_ref())
    }

    fn replace(&mut self, block: impl Block + 'static) {
        self.block = CachedBlock::new(block)
    }
}

#[derive(Default)]
pub struct Region {
    entries: Vec<BlockEntry>,
}

impl Region {
    pub fn push(&mut self, block: impl Block + 'static) {
        self.entries.push(BlockEntry::anonymous(block));
    }

    pub fn insert(&mut self, id: impl Into<String>, block: impl Block + 'static) {
        let id = id.into();

        match self.entry_mut(&id) {
            Some(entry) => entry.replace(block),
            None => self.entries.push(BlockEntry::identified(id, block)),
        }
    }

    pub fn get<B>(&self, id: impl AsRef<str>) -> Option<&B>
    where
        B: Block + 'static,
    {
        self.entry(id).and_then(|entry| entry.block.get())
    }

    pub fn get_mut<B>(&mut self, id: impl AsRef<str>) -> Option<&mut B>
    where
        B: Block + 'static,
    {
        self.entry_mut(id).and_then(|entry| entry.block.get_mut())
    }

    pub fn remove(&mut self, id: impl AsRef<str>) {
        if let Some(index) = self
            .entries
            .iter()
            .position(|entry| entry.match_id(id.as_ref()))
        {
            self.entries.remove(index);
        }
    }

    pub fn render(&mut self, width: usize) -> Vec<String> {
        self.entries
            .iter_mut()
            .flat_map(|entry| entry.block.render(width))
            .cloned()
            .collect()
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn mark_all_dirty(&mut self) {
        self.entries
            .iter_mut()
            .for_each(|entry| entry.block.mark_dirty());
    }

    pub fn mark_all_clean(&mut self) {
        self.entries
            .iter_mut()
            .for_each(|entry| entry.block.mark_clean());
    }

    fn entry(&self, id: impl AsRef<str>) -> Option<&BlockEntry> {
        self.entries
            .iter()
            .find(|entry| entry.match_id(id.as_ref()))
    }

    fn entry_mut(&mut self, id: impl AsRef<str>) -> Option<&mut BlockEntry> {
        self.entries
            .iter_mut()
            .find(|entry| entry.match_id(id.as_ref()))
    }
}
