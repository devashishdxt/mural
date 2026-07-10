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

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use std::rc::Rc;

    use super::{Frame, RenderedLines};

    fn chunk(lines: &[&str]) -> Rc<RenderedLines> {
        Rc::new(lines.iter().map(|line| (*line).to_owned()).collect())
    }

    #[test]
    fn collection_flattens_non_empty_chunks() {
        let frame: Frame = [chunk(&["a", "b"]), chunk(&[]), chunk(&["c"])]
            .into_iter()
            .collect();

        assert_eq!(frame.len(), 3);
        assert_eq!(frame.iter().collect::<Vec<_>>(), ["a", "b", "c"]);
        assert_eq!(&frame[0], "a");
        assert_eq!(&frame[1], "b");
        assert_eq!(&frame[2], "c");
    }

    #[test]
    fn extend_appends_chunks() {
        let mut frame: Frame = [chunk(&["a"])].into_iter().collect();
        let other: Frame = [chunk(&["b", "c"])].into_iter().collect();

        frame.extend(other);

        assert_eq!(frame.iter().collect::<Vec<_>>(), ["a", "b", "c"]);
    }

    #[test]
    fn equality_ignores_chunk_boundaries() {
        let left: Frame = [chunk(&["a", "b"]), chunk(&["c"])].into_iter().collect();
        let right: Frame = [chunk(&["a"]), chunk(&["b", "c"])].into_iter().collect();
        let different_length: Frame = [chunk(&["a", "b"])].into_iter().collect();
        let different_content: Frame = [chunk(&["a", "x", "c"])].into_iter().collect();

        assert_eq!(left, right);
        assert_ne!(left, different_length);
        assert_ne!(left, different_content);
    }

    #[test]
    #[should_panic(expected = "frame index out of bounds")]
    fn indexing_past_the_end_panics() {
        let frame: Frame = [chunk(&["a"])].into_iter().collect();
        let _ = &frame[1];
    }
}
