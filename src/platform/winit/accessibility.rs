//! One window's accessibility: its AccessKit adapter, the tree it last
//! published and the screen reader requests waiting for the next frame.
//!
//! AccessKit speaks UI Automation on Windows, AT-SPI on Linux and the BSDs and
//! NSAccessibility on macOS. Nothing is built until an assistive technology
//! asks for the tree, so a window no screen reader reads pays nothing.

use super::accessibility_tree::{request_for_action, tree_update};
use crate::core::ElementId;
use crate::core::accessibility::AccessibilityTree;
use crate::core::presenter::Presenter;
use crate::elements::element::Element;
use crate::platform::accessibility_request::AccessibilityRequest;
use accesskit::TreeUpdate;
use accesskit_winit::WindowEvent as AccessKitEvent;

/// What the platform holds: the tree, and the scale and window title it was
/// published with.
struct Published {
    tree: AccessibilityTree,
    scale_factor: f64,
    title: String,
}

pub(super) struct WindowAccessibility {
    /// None in tests, which read the updates instead.
    adapter: Option<accesskit_winit::Adapter>,
    /// An assistive technology asked for the tree and has not let go.
    active: bool,
    published: Option<Published>,
    requests: Vec<(ElementId, AccessibilityRequest)>,
}

impl WindowAccessibility {
    pub(super) fn new(adapter: Option<accesskit_winit::Adapter>) -> Self {
        Self {
            adapter,
            active: false,
            published: None,
            requests: Vec::new(),
        }
    }

    /// Let the adapter see a window event (its bounds and focus), before the
    /// window handles it.
    pub(super) fn process_window_event(
        &mut self,
        window: &winit::window::Window,
        event: &winit::event::WindowEvent,
    ) {
        if let Some(adapter) = &mut self.adapter {
            adapter.process_event(window, event);
        }
    }

    /// Take an AccessKit event for this window. Returns whether the window
    /// needs a frame to answer it.
    pub(super) fn handle_event(&mut self, event: AccessKitEvent) -> bool {
        match event {
            AccessKitEvent::InitialTreeRequested => {
                self.active = true;
                self.published = None;
                true
            }
            AccessKitEvent::ActionRequested(request) => match request_for_action(&request) {
                Some(request) => {
                    self.requests.push(request);
                    true
                }
                None => {
                    log::warn!(
                        "ignored winit accessibility action {:?} for node {:?}",
                        request.action,
                        request.target_node
                    );
                    false
                }
            },
            AccessKitEvent::AccessibilityDeactivated => {
                self.active = false;
                self.published = None;
                false
            }
        }
    }

    pub(super) fn has_requests(&self) -> bool {
        !self.requests.is_empty()
    }

    pub(super) fn take_requests(&mut self) -> Vec<(ElementId, AccessibilityRequest)> {
        std::mem::take(&mut self.requests)
    }

    /// The tree the platform holds, which requests are checked against.
    pub(super) fn published_tree(&self) -> Option<&AccessibilityTree> {
        self.published.as_ref().map(|published| &published.tree)
    }

    /// Whether an assistive technology asked for the tree and has not let go.
    pub(super) fn is_reading(&self) -> bool {
        self.active
    }

    /// Whether an assistive technology is reading and has no tree yet.
    pub(super) fn needs_tree(&self) -> bool {
        self.active && self.published.is_none()
    }

    /// Publish the presenter's tree under a window node titled `title` if an
    /// assistive technology is reading and it changed since the last publish.
    pub(super) fn publish<E: Element>(
        &mut self,
        presenter: &Presenter<E>,
        title: &str,
        scale_factor: f64,
    ) {
        if !self.active {
            return;
        }
        let tree = match presenter.accessibility_tree() {
            Ok(tree) => tree,
            Err(err) => {
                log::error!("failed to build winit accessibility tree: {err}");
                return;
            }
        };
        if let Some(update) = self.next_update(tree, title, scale_factor)
            && let Some(adapter) = &mut self.adapter
        {
            adapter.update_if_active(|| update);
        }
    }

    /// The update that brings the platform to `tree`, or None when no
    /// assistive technology is reading or it already holds the tree. Every
    /// update carries the whole tree.
    pub(super) fn next_update(
        &mut self,
        tree: AccessibilityTree,
        title: &str,
        scale_factor: f64,
    ) -> Option<TreeUpdate> {
        let unchanged = self.published.as_ref().is_some_and(|published| {
            published.tree == tree
                && published.scale_factor == scale_factor
                && published.title == title
        });
        if !self.active || unchanged {
            return None;
        }
        let update = tree_update(&tree, title, scale_factor);
        self.published = Some(Published {
            tree,
            scale_factor,
            title: title.to_string(),
        });
        Some(update)
    }
}
