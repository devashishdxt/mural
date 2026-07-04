use std::{borrow::Cow, cell::RefCell, rc::Rc};

use crate::Block;

#[derive(Clone, Default)]
pub(crate) struct WidthRecordingBlock {
    widths: Rc<RefCell<Vec<usize>>>,
}

impl WidthRecordingBlock {
    pub(crate) fn widths(&self) -> Vec<usize> {
        self.widths.borrow().clone()
    }
}

impl Block for WidthRecordingBlock {
    fn render(&self, width: usize) -> Vec<Cow<'_, str>> {
        self.widths.borrow_mut().push(width);
        vec![Cow::Borrowed("width")]
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(crate) struct NamedBlock(pub(crate) &'static str);

impl Block for NamedBlock {
    fn render(&self, _width: usize) -> Vec<Cow<'_, str>> {
        vec![Cow::Borrowed(self.0)]
    }
}

pub(crate) struct OtherBlock;

impl Block for OtherBlock {
    fn render(&self, _width: usize) -> Vec<Cow<'_, str>> {
        vec![Cow::Borrowed("other")]
    }
}

#[derive(Clone)]
pub(crate) struct CountingBlock {
    text: Rc<RefCell<String>>,
    renders: Rc<RefCell<usize>>,
    every_frame: bool,
}

impl CountingBlock {
    pub(crate) fn new(text: &str) -> Self {
        Self {
            text: Rc::new(RefCell::new(text.to_owned())),
            renders: Rc::new(RefCell::new(0)),
            every_frame: false,
        }
    }

    pub(crate) fn every_frame(text: &str) -> Self {
        Self {
            every_frame: true,
            ..Self::new(text)
        }
    }

    pub(crate) fn render_count(&self) -> usize {
        *self.renders.borrow()
    }

    pub(crate) fn set_text(&self, text: &str) {
        *self.text.borrow_mut() = text.to_owned();
    }
}

impl Block for CountingBlock {
    fn render(&self, _width: usize) -> Vec<Cow<'_, str>> {
        *self.renders.borrow_mut() += 1;
        vec![Cow::Owned(self.text.borrow().clone())]
    }

    fn render_every_frame(&self) -> bool {
        self.every_frame
    }
}
