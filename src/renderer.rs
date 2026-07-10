use crate::{
    backend::{Backend, ExecuteOp},
    differ::{Differ, MyersDiffer},
    planner::{DefaultPlanner, Plan, Planner},
};

pub struct Renderer<'a, 'b> {
    current_frame: &'b [String],
    new_frame: &'a [String],
    height: usize,
    sentinel_row: usize,
}

impl<'a, 'b> Renderer<'a, 'b> {
    pub fn new(
        current_frame: &'b [String],
        new_frame: &'a [String],
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
