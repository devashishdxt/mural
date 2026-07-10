use std::rc::Rc;

use crate::{
    block::{Block, ErasedBlock},
    frame::{Frame, RenderedLines},
};

struct CachedBlock {
    block: Box<dyn ErasedBlock>,
    dirty: bool,
    width: Option<usize>,
    lines: Rc<RenderedLines>,
}

impl CachedBlock {
    fn new(block: impl Block + 'static) -> Self {
        Self {
            block: Box::new(block),
            dirty: true,
            width: None,
            lines: Rc::new(Vec::new()),
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

    fn render(&mut self, width: usize) -> Rc<RenderedLines> {
        if self.should_render(width) {
            self.refresh(width);
        }

        Rc::clone(&self.lines)
    }

    fn refresh(&mut self, width: usize) {
        let rendered_lines = self.block.render(width);

        debug_assert!(
            rendered_lines
                .iter()
                .all(|line| !line.contains('\n') && !line.contains('\r'))
        );

        self.lines = Rc::new(
            rendered_lines
                .into_iter()
                .map(|line| line.into_owned())
                .collect(),
        );
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

    pub fn render(&mut self, width: usize) -> Frame {
        self.entries
            .iter_mut()
            .map(|entry| entry.block.render(width))
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

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use std::{borrow::Cow, cell::Cell, rc::Rc};

    use super::Region;
    use crate::block::Block;

    struct CountingBlock {
        text: String,
        renders: Rc<Cell<usize>>,
        every_frame: bool,
    }

    impl CountingBlock {
        fn new(text: &str, renders: &Rc<Cell<usize>>) -> Self {
            Self {
                text: text.to_owned(),
                renders: Rc::clone(renders),
                every_frame: false,
            }
        }
    }

    impl Block for CountingBlock {
        fn render(&self, width: usize) -> Vec<Cow<'_, str>> {
            self.renders.set(self.renders.get() + 1);
            vec![Cow::Owned(format!("{}:{width}", self.text))]
        }

        fn render_every_frame(&self) -> bool {
            self.every_frame
        }
    }

    #[test]
    fn cached_blocks_render_only_when_needed() {
        let renders = Rc::new(Cell::new(0));
        let mut region = Region::default();
        region.push(CountingBlock::new("block", &renders));

        assert_eq!(region.render(10).iter().collect::<Vec<_>>(), ["block:10"]);
        assert_eq!(renders.get(), 1);

        region.mark_all_clean();
        region.render(10);
        assert_eq!(renders.get(), 1);

        region.render(20);
        assert_eq!(renders.get(), 2);

        region.mark_all_dirty();
        region.render(20);
        assert_eq!(renders.get(), 3);
    }

    #[test]
    fn blocks_can_request_rendering_every_frame() {
        let renders = Rc::new(Cell::new(0));
        let mut block = CountingBlock::new("dynamic", &renders);
        block.every_frame = true;
        let mut region = Region::default();
        region.push(block);

        region.render(10);
        region.mark_all_clean();
        region.render(10);

        assert_eq!(renders.get(), 2);
    }

    #[test]
    fn mark_clean_before_first_render_keeps_block_dirty() {
        let renders = Rc::new(Cell::new(0));
        let mut region = Region::default();
        region.push(CountingBlock::new("block", &renders));

        region.mark_all_clean();
        region.render(10);
        region.render(10);

        assert_eq!(renders.get(), 2);
    }

    #[test]
    fn identified_blocks_support_lookup_and_mutation() {
        let renders = Rc::new(Cell::new(0));
        let mut region = Region::default();
        region.insert("status", CountingBlock::new("before", &renders));
        region.render(10);
        region.mark_all_clean();

        assert_eq!(
            region.get::<CountingBlock>("status").unwrap().text,
            "before"
        );
        assert!(region.get::<String>("status").is_none());
        assert!(region.get::<CountingBlock>("missing").is_none());
        assert!(region.get_mut::<String>("status").is_none());

        region.get_mut::<CountingBlock>("status").unwrap().text = "after".to_owned();
        assert_eq!(region.render(10).iter().collect::<Vec<_>>(), ["after:10"]);
        assert_eq!(renders.get(), 2);
    }

    #[test]
    fn inserting_an_existing_id_replaces_it_in_place() {
        let renders = Rc::new(Cell::new(0));
        let mut region = Region::default();
        region.push("first");
        region.insert("status", CountingBlock::new("old", &renders));
        region.insert("status", String::from("new"));

        assert!(region.get::<CountingBlock>("status").is_none());
        assert_eq!(region.get::<String>("status").unwrap(), "new");
        assert_eq!(
            region.render(20).iter().collect::<Vec<_>>(),
            ["first", "new"]
        );
    }

    #[test]
    fn remove_and_clear_discard_entries() {
        let mut region = Region::default();
        region.push("anonymous");
        region.insert("one", "identified");
        region.insert("two", "another");

        region.remove("missing");
        region.remove("one");
        assert_eq!(
            region.render(20).iter().collect::<Vec<_>>(),
            ["anonymous", "another"]
        );

        region.clear();
        assert_eq!(region.render(20).len(), 0);
    }
}
