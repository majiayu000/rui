//! Pairing freshly built children with the previous ones, by key or position.

use super::AnyElement;
use crate::core::ElementId;
use crate::core::entity::EntityId;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ReconcileKey {
    Id(ElementId),
    View(EntityId),
}

/// Reconcile freshly built `children` with the `previous` ones, as
/// [`AnyElement::reconcile`] does for each pair. Children with a key (an
/// explicit id, or the view they show) pair with the previous child of the same
/// key wherever it was; the others pair by position. Returns true when every
/// child pairs in place and is kept, so the parent can keep `previous` whole.
pub(crate) fn reconcile_children(children: &mut [AnyElement], previous: &mut [AnyElement]) -> bool {
    let mut same = children.len() == previous.len();
    // Pair in place while the keys agree, which is all of a list that did not move.
    let mut index = 0;
    while index < children.len().min(previous.len())
        && children[index].reconcile_key() == previous[index].reconcile_key()
    {
        same &= children[index].reconcile(&mut previous[index]);
        index += 1;
    }
    if index == children.len() {
        return same;
    }
    let mut keyed: HashMap<ReconcileKey, usize> = previous[index..]
        .iter()
        .enumerate()
        .filter_map(|(offset, old)| old.reconcile_key().map(|key| (key, index + offset)))
        .collect();
    for (at, child) in children.iter_mut().enumerate().skip(index) {
        let old = match child.reconcile_key() {
            Some(key) => keyed.remove(&key),
            None => previous
                .get(at)
                .is_some_and(|old| old.reconcile_key().is_none())
                .then_some(at),
        };
        match old {
            Some(old) => {
                let kept = child.reconcile(&mut previous[old]);
                same &= kept && old == at;
            }
            None => same = false,
        }
    }
    same
}

impl AnyElement {
    /// What pairs this element with the one it replaces when its siblings
    /// move: its explicit id, or the view it shows.
    fn reconcile_key(&self) -> Option<ReconcileKey> {
        if !self.paint_record.keyed {
            return None;
        }
        match self.inner.id() {
            Some(id) if !id.is_ephemeral() => Some(ReconcileKey::Id(id)),
            _ => crate::elements::view_element::view_of(&*self.inner).map(ReconcileKey::View),
        }
    }
}
