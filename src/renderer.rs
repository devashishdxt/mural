use crate::{
    backend::{Backend, ExecuteOp},
    differ::{Differ, MyersDiffer},
    frame::Frame,
    planner::{DefaultPlanner, Plan, Planner},
};

pub struct Renderer<'a, 'b> {
    current_frame: &'b Frame,
    new_frame: &'a Frame,
    height: usize,
    sentinel_row: usize,
}

impl<'a, 'b> Renderer<'a, 'b> {
    pub fn new(
        current_frame: &'b Frame,
        new_frame: &'a Frame,
        height: usize,
        sentinel_row: usize,
    ) -> Self {
        Self {
            current_frame,
            new_frame,
            height,
            sentinel_row,
        }
    }

    pub fn render<B>(self, backend: &mut B, force_full_redraw: bool) -> Result<usize, B::Error>
    where
        B: Backend,
    {
        let plan = if force_full_redraw {
            Plan::full_redraw(self.new_frame, self.height)
        } else {
            let diff = MyersDiffer
                .diff(self.current_frame, self.new_frame)
                .normalize();

            DefaultPlanner::new(
                self.current_frame.len(),
                self.new_frame,
                self.height,
                self.sentinel_row,
            )
            .plan(diff)
        };

        for render_op in plan.render_ops() {
            backend.execute(*render_op)?;
        }

        backend.flush()?;

        Ok(plan.final_sentinel_row())
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use std::{io, rc::Rc};

    use super::Renderer;
    use crate::{
        backend::Backend,
        frame::{Frame, RenderedLines},
    };

    #[derive(Default)]
    struct MockBackend {
        calls: Vec<String>,
        fail_on: Option<&'static str>,
    }

    impl MockBackend {
        fn call(&mut self, name: &'static str) -> io::Result<()> {
            self.calls.push(name.to_owned());
            if self.fail_on == Some(name) {
                Err(io::Error::other(name))
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
        fn move_up(&mut self, _: usize) -> Result<(), Self::Error> {
            self.call("move_up")
        }
        fn move_down(&mut self, _: usize) -> Result<(), Self::Error> {
            self.call("move_down")
        }
        fn carriage_return(&mut self) -> Result<(), Self::Error> {
            self.call("carriage_return")
        }
        fn newline(&mut self) -> Result<(), Self::Error> {
            self.call("newline")
        }
        fn scroll_up(&mut self, _: usize) -> Result<(), Self::Error> {
            self.call("scroll_up")
        }
        fn insert_lines(&mut self, _: usize) -> Result<(), Self::Error> {
            self.call("insert_lines")
        }
        fn delete_lines(&mut self, _: usize) -> Result<(), Self::Error> {
            self.call("delete_lines")
        }
        fn clear_line(&mut self) -> Result<(), Self::Error> {
            self.call("clear_line")
        }
        fn write_str(&mut self, _: &str) -> Result<(), Self::Error> {
            self.call("write_str")
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

    fn frame(lines: &[&str]) -> Frame {
        [Rc::new(
            lines
                .iter()
                .map(|line| (*line).to_owned())
                .collect::<RenderedLines>(),
        )]
        .into_iter()
        .collect()
    }

    #[test]
    fn forced_render_executes_a_full_redraw_and_flushes() {
        let current = Frame::default();
        let new = frame(&["line"]);
        let mut backend = MockBackend::default();

        let sentinel = Renderer::new(&current, &new, 4, 0)
            .render(&mut backend, true)
            .unwrap();

        assert_eq!(sentinel, 1);
        assert_eq!(
            backend.calls,
            [
                "clear_screen",
                "purge_scrollback",
                "move_to_top_left",
                "write_str",
                "carriage_return",
                "newline",
                "flush",
            ]
        );
    }

    #[test]
    fn unchanged_render_only_flushes() {
        let current = frame(&["same"]);
        let new = frame(&["same"]);
        let mut backend = MockBackend::default();

        let sentinel = Renderer::new(&current, &new, 4, 1)
            .render(&mut backend, false)
            .unwrap();

        assert_eq!(sentinel, 1);
        assert_eq!(backend.calls, ["flush"]);
    }

    #[test]
    fn backend_operation_and_flush_errors_are_propagated() {
        let current = Frame::default();
        let new = frame(&["line"]);
        let mut operation_failure = MockBackend {
            fail_on: Some("clear_screen"),
            ..Default::default()
        };
        assert!(
            Renderer::new(&current, &new, 4, 0)
                .render(&mut operation_failure, true)
                .is_err()
        );
        assert_eq!(operation_failure.calls, ["clear_screen"]);

        let mut flush_failure = MockBackend {
            fail_on: Some("flush"),
            ..Default::default()
        };
        assert!(
            Renderer::new(&current, &current, 4, 0)
                .render(&mut flush_failure, false)
                .is_err()
        );
        assert_eq!(flush_failure.calls, ["flush"]);
    }
}
