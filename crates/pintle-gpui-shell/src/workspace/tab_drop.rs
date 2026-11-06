//! Tab-bar geometry only; Kit remains responsible for moving retained panels.
use super::*;
use gpui::component::dock::DragPanel;
use gpui::{Bounds, Pixels, Point};

#[derive(Default)]
pub(super) struct TabDropState {
    pub viewport: Option<Bounds<Pixels>>,
    pub tabs: Vec<(PanelId, Option<Bounds<Pixels>>)>,
    pub drag: Option<DragPanel>,
}
#[derive(Clone, Copy)]
pub(super) struct Insertion {
    pub index: usize,
    pub edge: Pixels,
}
impl TabDropState {
    pub fn target(
        &self,
        node: NodeId,
        drag: &DragPanel,
        pointer: Point<Pixels>,
    ) -> Option<Insertion> {
        if !self.viewport?.contains(&pointer) {
            return None;
        }
        let (boundary, edge) = self
            .tabs
            .iter()
            .enumerate()
            .find_map(|(ix, (_, bounds))| {
                let bounds = bounds.as_ref()?;
                (pointer.x < bounds.right()).then(|| {
                    if pointer.x < bounds.center().x {
                        (ix, bounds.left())
                    } else {
                        (ix + 1, bounds.right())
                    }
                })
            })
            .or_else(|| {
                self.tabs
                    .last()
                    .and_then(|(_, bounds)| bounds.as_ref())
                    .filter(|b| pointer.x >= b.right())
                    .map(|b| (self.tabs.len(), b.right()))
            })?;
        let source = (drag.source() == node)
            .then(|| self.tabs.iter().position(|(id, _)| *id == drag.panel()))
            .flatten();
        let index = insertion_index(boundary, source)?;
        Some(Insertion { index, edge })
    }
}
// Kit's slot is measured after detaching the source, not in the old order.
fn insertion_index(boundary: usize, source: Option<usize>) -> Option<usize> {
    let index = boundary - usize::from(source.is_some_and(|source| source < boundary));
    (source != Some(index)).then_some(index)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn insertion_slots_account_for_detachment_and_noops() {
        assert_eq!(insertion_index(0, Some(2)), Some(0));
        assert_eq!(insertion_index(2, Some(0)), Some(1));
        assert_eq!(insertion_index(3, Some(0)), Some(2));
        assert_eq!(insertion_index(1, Some(1)), None);
        assert_eq!(insertion_index(2, Some(1)), None);
        assert_eq!(insertion_index(3, Some(2)), None);
        assert_eq!(insertion_index(0, None), Some(0));
        assert_eq!(insertion_index(3, None), Some(3));
    }
}
