//! Gesture (mouse) based movement

use xi_core_lib::rpc::{GestureType, SelectionGranularity};
use xi_core_lib::selection::{SelRegion, Selection};
use xi_core_lib::word_boundaries::WordCursor;
use xi_rope::interval::IntervalBounds;
use xi_rope::Rope;

/// State required to resolve a drag gesture into a selection.
pub(crate) struct DragState {
    /// All the selection regions other than the one being dragged.
    base_sel: Selection,

    /// Start of the region selected when drag was started (region is
    /// assumed to be forward).
    min: usize,

    /// End of the region selected when drag was started.
    max: usize,

    granularity: SelectionGranularity,
}

pub(crate) fn region_for_gesture(
    text: &Rope,
    offset: usize,
    granularity: SelectionGranularity,
) -> SelRegion {
    match granularity {
        SelectionGranularity::Point => SelRegion::caret(offset),
        SelectionGranularity::Word => {
            let mut word_cursor = WordCursor::new(text, offset);
            let (start, end) = word_cursor.select_word();
            SelRegion::new(start, end)
        }
        SelectionGranularity::Line => {
            let line = text.line_of_offset(offset);
            let start = text.offset_of_line(line);
            let end = text.offset_of_line(line + 1);
            SelRegion::new(start, end)
        }
    }
}

/// Calculates the region generated by extending (via shift-click or drag, e.g)
/// an existing region.
fn region_extending_region<IV: IntervalBounds>(
    text: &Rope,
    active_region_interval: IV,
    offset: usize,
    granularity: SelectionGranularity,
) -> SelRegion {
    let active = active_region_interval.into_interval(text.len());
    let extension = region_for_gesture(text, offset, granularity);

    if offset >= active.start {
        SelRegion::new(active.start, extension.end)
    } else {
        SelRegion::new(active.start, extension.start)
    }
}

pub(crate) struct GestureContext<'a> {
    text: &'a Rope,
    sel: &'a Selection,
    drag_state: &'a mut Option<DragState>,
}

impl<'a> GestureContext<'a> {
    pub(crate) fn new(
        text: &'a Rope,
        sel: &'a Selection,
        drag_state: &'a mut Option<DragState>,
    ) -> Self {
        GestureContext { text, sel, drag_state }
    }

    pub(crate) fn selection_for_gesture(
        &mut self,
        offset: usize,
        gesture: GestureType,
    ) -> Selection {
        if let GestureType::Select { granularity: SelectionGranularity::Point, multi: true } =
            gesture
        {
            if !self.sel.regions_in_range(offset, offset).is_empty() && self.sel.len() > 1 {
                // we don't allow toggling the last selection
                let mut new = self.sel.clone();
                new.delete_range(offset, offset, true);
                return new;
            }
        }

        match gesture {
            GestureType::Select { granularity, multi } => {
                let new_region = region_for_gesture(&self.text, offset, granularity);
                let new_sel = if multi {
                    let mut new = self.sel.clone();
                    new.add_region(new_region);
                    new
                } else {
                    new_region.into()
                };

                *(self.drag_state) = Some(DragState {
                    base_sel: new_sel.clone(),
                    min: new_region.start,
                    max: new_region.end,
                    granularity,
                });
                new_sel
            }
            GestureType::SelectExtend { granularity } => {
                if self.sel.len() == 0 {
                    return self.sel.clone();
                }
                let active_region = self.sel.last().clone().unwrap();
                let new_region = region_for_gesture(self.text, offset, granularity);
                let merged_region = if offset >= new_region.start {
                    SelRegion::new(active_region.start, new_region.end)
                } else {
                    SelRegion::new(active_region.start, new_region.start)
                };
                let mut new = self.sel.clone();
                new.add_region(merged_region);
                *(self.drag_state) = Some(DragState {
                    base_sel: new.clone(),
                    min: new_region.start,
                    max: new_region.end,
                    granularity,
                });

                new
            }
            GestureType::Drag => {
                let new_sel = self.drag_state.as_ref().map(|drag_state| {
                    let mut sel = drag_state.base_sel.clone();
                    let new_region = region_extending_region(
                        &self.text,
                        drag_state.min..drag_state.max,
                        offset,
                        drag_state.granularity,
                    );
                    sel.add_region(new_region.with_horiz(None));
                    sel
                });

                new_sel.unwrap_or_else(|| self.sel.clone())
            }
            _other => panic!("unexpected gesture type {:?}", _other),
        }
    }
}