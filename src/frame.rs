use std::{ops::Index, rc::Rc};

pub type RenderedLines = Vec<String>;

#[derive(Debug, Default)]
pub struct Frame {
    chunks: Vec<Rc<RenderedLines>>,
    starts: Vec<usize>,
    len: usize,
}

impl Frame {
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.chunks
            .iter()
            .flat_map(|chunk| chunk.iter().map(String::as_str))
    }

    pub fn extend(&mut self, other: Self) {
        for chunk in other.chunks {
            self.push(chunk);
        }
    }

    fn push(&mut self, chunk: Rc<RenderedLines>) {
        if chunk.is_empty() {
            return;
        }

        self.starts.push(self.len);
        self.len += chunk.len();
        self.chunks.push(chunk);
    }
}

impl FromIterator<Rc<RenderedLines>> for Frame {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = Rc<RenderedLines>>,
    {
        let mut frame = Self::default();

        for chunk in iter {
            frame.push(chunk);
        }

        frame
    }
}

impl Index<usize> for Frame {
    type Output = str;

    fn index(&self, index: usize) -> &Self::Output {
        assert!(index < self.len, "frame index out of bounds");

        let chunk_index = self.starts.partition_point(|&start| start <= index) - 1;
        let line_index = index - self.starts[chunk_index];

        self.chunks[chunk_index][line_index].as_str()
    }
}

impl PartialEq for Frame {
    fn eq(&self, other: &Self) -> bool {
        self.len == other.len && self.iter().eq(other.iter())
    }
}

impl Eq for Frame {}
