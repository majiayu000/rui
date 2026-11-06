//! Reacting to other entities: observers of `notify` and subscribers to emitted events.

use crate::core::app::AppContext;
use crate::core::entity::{Entity, EntityId};
use std::any::{Any, TypeId};
use std::cell::Cell;
use std::rc::Rc;

/// Rounds of effects one flush runs before leaving the rest to the next frame, so
/// an observer that notifies what it observes cannot hang the UI thread.
const MAX_FLUSH_ROUNDS: usize = 64;

type ObserverFn = Box<dyn FnMut(&mut AppContext)>;
type SubscriberFn = Box<dyn FnMut(&dyn Any, &mut AppContext)>;

enum Effect {
    Notify(EntityId),
    Emit(EntityId, TypeId, Box<dyn Any>),
}

struct Listener<F> {
    entity: EntityId,
    alive: Rc<Cell<bool>>,
    callback: F,
}

#[derive(Default)]
pub(crate) struct Effects {
    queue: Vec<Effect>,
    observers: Vec<Listener<ObserverFn>>,
    subscribers: Vec<(TypeId, Listener<SubscriberFn>)>,
}

impl Effects {
    pub(crate) fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub(crate) fn push_notify(&mut self, entity: EntityId) {
        self.queue.push(Effect::Notify(entity));
    }
}

/// Keeps an observer or subscriber registered; dropping it unregisters.
#[must_use = "dropping a Subscription unregisters it"]
pub struct Subscription {
    alive: Rc<Cell<bool>>,
}

impl Subscription {
    /// Keep the registration for as long as the observed entity exists.
    pub fn detach(self) {
        std::mem::forget(self);
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.alive.set(false);
    }
}

fn listener<F>(entity: EntityId, callback: F) -> (Listener<F>, Subscription) {
    let alive = Rc::new(Cell::new(true));
    let subscription = Subscription {
        alive: Rc::clone(&alive),
    };
    (
        Listener {
            entity,
            alive,
            callback,
        },
        subscription,
    )
}

impl AppContext {
    /// Call `callback` after each `notify` of `entity`, before the next frame.
    pub fn observe<T: 'static>(
        &mut self,
        entity: Entity<T>,
        mut callback: impl FnMut(Entity<T>, &mut AppContext) + 'static,
    ) -> Subscription {
        let (listener, subscription) = listener(
            entity.id(),
            Box::new(move |cx: &mut AppContext| callback(entity, cx)) as ObserverFn,
        );
        self.effects.observers.push(listener);
        subscription
    }

    /// Call `callback` with each `E` that `entity` emits, before the next frame.
    pub fn subscribe<T: 'static, E: 'static>(
        &mut self,
        entity: Entity<T>,
        mut callback: impl FnMut(Entity<T>, &E, &mut AppContext) + 'static,
    ) -> Subscription {
        let (listener, subscription) = listener(
            entity.id(),
            Box::new(move |event: &dyn Any, cx: &mut AppContext| {
                if let Some(event) = event.downcast_ref::<E>() {
                    callback(entity, event, cx);
                }
            }) as SubscriberFn,
        );
        self.effects.subscribers.push((TypeId::of::<E>(), listener));
        subscription
    }

    /// Send `event` to every subscriber of `entity` for `E`.
    pub fn emit<T: 'static, E: 'static>(&mut self, entity: Entity<T>, event: E) {
        self.effects.queue.push(Effect::Emit(
            entity.id(),
            TypeId::of::<E>(),
            Box::new(event),
        ));
    }

    /// Remove `entity`, and with it every observer and subscriber of it. A
    /// view made with [`AppContext::new_view`] is dropped with what it read
    /// and its queued listener updates.
    pub fn release<T: 'static>(&mut self, entity: Entity<T>) -> bool {
        self.release_view(entity.id()) || self.entities.remove(entity.id())
    }

    /// Run queued notifications and events, including ones they cause.
    pub(crate) fn flush_effects(&mut self) {
        for _ in 0..MAX_FLUSH_ROUNDS {
            let queue = std::mem::take(&mut self.effects.queue);
            if queue.is_empty() {
                return;
            }
            for effect in queue {
                self.dispatch_effect(effect);
            }
        }
        if !self.effects.queue.is_empty() {
            log::error!(
                "entity effects still pending after {MAX_FLUSH_ROUNDS} rounds; an observer may be notifying what it observes"
            );
            self.mark_redraw_from(crate::core::app::RedrawSource::ViewNotification);
        }
    }

    fn dispatch_effect(&mut self, effect: Effect) {
        let entities = &self.entities;
        let live =
            |entity: EntityId, alive: &Rc<Cell<bool>>| alive.get() && entities.contains(entity);
        self.effects.observers.retain(|o| live(o.entity, &o.alive));
        self.effects
            .subscribers
            .retain(|(_, s)| live(s.entity, &s.alive));

        match effect {
            Effect::Notify(entity) => {
                // Taken out so callbacks get `&mut AppContext`; ones they add are kept.
                let mut observers = std::mem::take(&mut self.effects.observers);
                for observer in observers.iter_mut().filter(|o| o.entity == entity) {
                    if observer.alive.get() {
                        (observer.callback)(self);
                    }
                }
                observers.append(&mut self.effects.observers);
                self.effects.observers = observers;
            }
            Effect::Emit(entity, event_type, event) => {
                let mut subscribers = std::mem::take(&mut self.effects.subscribers);
                for (_, subscriber) in subscribers
                    .iter_mut()
                    .filter(|(ty, s)| *ty == event_type && s.entity == entity)
                {
                    if subscriber.alive.get() {
                        (subscriber.callback)(event.as_ref(), self);
                    }
                }
                subscribers.append(&mut self.effects.subscribers);
                self.effects.subscribers = subscribers;
            }
        }
    }
}
