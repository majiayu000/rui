//! Hover and press carried over to the control rebuilt in a control's place,
//! so a press survives a rebuild between mouse-down and mouse-up and the
//! release still activates it.

use super::state::{IndexedInteractionState, InteractionState};
use super::{
    Button, Checkbox, DataList, DataTableRow, DataTree, Menu, RadioGroup, SegmentedControl, Select,
    Switch, TabList,
};
use crate::elements::element::StableId;
use std::any::{Any, TypeId};

/// A control that keeps its own hover and press.
pub(crate) trait PressState: Any {
    /// Its id, its interaction state, and the per-item state of controls
    /// with several items.
    fn press_state(
        &mut self,
    ) -> (
        StableId,
        &mut InteractionState,
        Option<&mut IndexedInteractionState>,
    );
}

/// Carry hover and press from `previous` to `next` when both are a `T`.
/// The frame pairs them by explicit id, or by place among unkeyed siblings,
/// which is where the stable identity comes from; an explicit id must also
/// match. Returns whether both were a `T`, which `kind` says first so most
/// elements are passed by without a downcast.
fn carry<T: PressState>(kind: TypeId, next: &mut dyn Any, previous: &mut dyn Any) -> bool {
    if kind != TypeId::of::<T>() {
        return false;
    }
    let (Some(next), Some(previous)) = (next.downcast_mut::<T>(), previous.downcast_mut::<T>())
    else {
        return false;
    };
    let (old_id, old_state, old_items) = previous.press_state();
    let (old_state, old_items) = (*old_state, old_items.map(|items| *items));
    let (id, state, items) = next.press_state();
    let same = id.is_explicit() == old_id.is_explicit() && (!id.is_explicit() || id == old_id);
    if same && state.can_activate() {
        state.set_hovered(old_state.hovered());
        state.set_pressed(old_state.pressed());
        if let (Some(items), Some(old_items)) = (items, old_items) {
            *items = old_items;
        }
    }
    true
}

/// [`carry`] for every control that keeps its own hover and press, given
/// the `kind` both elements are.
pub(crate) fn carry_press(kind: TypeId, next: &mut dyn Any, previous: &mut dyn Any) {
    let _ = carry::<Button>(kind, next, previous)
        || carry::<Checkbox>(kind, next, previous)
        || carry::<Switch>(kind, next, previous)
        || carry::<Select>(kind, next, previous)
        || carry::<RadioGroup>(kind, next, previous)
        || carry::<SegmentedControl>(kind, next, previous)
        || carry::<TabList>(kind, next, previous)
        || carry::<Menu>(kind, next, previous)
        || carry::<DataList>(kind, next, previous)
        || carry::<DataTree>(kind, next, previous)
        || carry::<DataTableRow>(kind, next, previous);
}
