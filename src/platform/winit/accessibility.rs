//! One window's accessibility: its AccessKit adapter, the tree it last
//! published and the screen reader requests waiting for the next frame.
//!
//! AccessKit speaks UI Automation on Windows, AT-SPI on Linux and the BSDs and
//! NSAccessibility on macOS. Nothing is built until an assistive technology
//! asks for the tree, so a window no screen reader reads pays nothing.

use super::accessibility_announce::{Announcement, Announcer, add_announcement};
use super::accessibility_diff::{HeldNodes, changes};
use super::accessibility_tree::{TextGeometry, request_for_action, tree_update};
use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAnnouncementKind, AccessibilityNode, AccessibilityRole, AccessibilityTree,
};
use crate::core::presenter::Presenter;
use crate::elements::element::Element;
use crate::platform::accessibility_request::AccessibilityRequest;
use accesskit::TreeUpdate;
use accesskit_winit::WindowEvent as AccessKitEvent;

/// What the platform holds: the tree, the text layouts, and the scale and
/// window title it was published with.
struct Published {
    tree: AccessibilityTree,
    text: TextGeometry,
    scale_factor: f64,
    title: String,
    announcement: Option<Announcement>,
    /// The AccessKit nodes the platform holds, which the next update is
    /// a diff against.
    nodes: HeldNodes,
}

pub(super) struct WindowAccessibility {
    /// None in tests, which read the updates instead.
    adapter: Option<accesskit_winit::Adapter>,
    /// An assistive technology asked for the tree and has not let go.
    active: bool,
    published: Option<Published>,
    requests: Vec<(ElementId, AccessibilityRequest)>,
    announcer: Announcer,
}

impl WindowAccessibility {
    pub(super) fn new(adapter: Option<accesskit_winit::Adapter>) -> Self {
        Self {
            adapter,
            active: false,
            published: None,
            requests: Vec::new(),
            announcer: Announcer::default(),
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
                log::info!("an assistive technology asked for the winit accessibility tree");
                self.active = true;
                self.published = None;
                self.announcer.clear();
                true
            }
            AccessKitEvent::ActionRequested(request) => {
                match request_for_action(&request, self.published_tree()) {
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
                }
            }
            AccessKitEvent::AccessibilityDeactivated => {
                self.active = false;
                self.published = None;
                self.announcer.clear();
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

    /// Take the action feedback ("Save activated") the presenter's elements
    /// queued, to say with the next update. Ignored while nothing reads.
    pub(super) fn hear<E>(&mut self, presenter: &mut Presenter<E>) {
        let feedback = presenter
            .take_accessibility_announcements()
            .into_iter()
            .filter(|announcement| {
                announcement.kind() == AccessibilityAnnouncementKind::ActionFeedback
            })
            .map(|announcement| announcement.message().to_string());
        if self.active {
            self.announcer.say(feedback);
        }
    }

    /// Whether an assistive technology asked for the tree and has not let go.
    pub(super) fn is_reading(&self) -> bool {
        self.active
    }

    /// Whether an assistive technology is reading and has no tree yet.
    pub(super) fn needs_tree(&self) -> bool {
        self.active && self.published.is_none()
    }

    /// Publish the presenter's tree under a window node titled `title`, with
    /// the action feedback its elements queued, if an assistive technology is
    /// reading and either changed since the last publish. Focus changes need
    /// no announcement: the tree's focus carries them.
    pub(super) fn publish<E: Element>(
        &mut self,
        presenter: &mut Presenter<E>,
        title: &str,
        scale_factor: f64,
    ) {
        if !self.active {
            return;
        }
        self.hear(presenter);
        let tree = match presenter.accessibility_tree() {
            Ok(tree) => tree,
            Err(err) => {
                log::error!("failed to build winit accessibility tree: {err}");
                return;
            }
        };
        let text = text_geometry(presenter, &tree);
        if let Some(update) = self.next_update(tree, text, title, scale_factor)
            && let Some(adapter) = &mut self.adapter
        {
            adapter.update_if_active(|| update);
        }
    }

    /// The update that brings the platform to `tree`, or None when no
    /// assistive technology is reading or it already holds the tree. The
    /// first update carries the whole tree, later ones only the nodes that
    /// changed.
    pub(super) fn next_update(
        &mut self,
        tree: AccessibilityTree,
        text: TextGeometry,
        title: &str,
        scale_factor: f64,
    ) -> Option<TreeUpdate> {
        let unchanged = self.published.as_ref().is_some_and(|published| {
            published.tree == tree
                && published.text == text
                && published.scale_factor == scale_factor
                && published.title == title
                && published.announcement.as_ref() == self.announcer.current()
        });
        if !self.active || unchanged {
            return None;
        }
        let mut full = tree_update(&tree, &text, title, scale_factor);
        let announcement = self.announcer.current().cloned();
        if let Some(announcement) = &announcement {
            add_announcement(&mut full, announcement);
        }
        let (update, nodes) = changes(full, self.published.as_ref().map(|held| &held.nodes));
        self.published = Some(Published {
            tree,
            text,
            scale_factor,
            title: title.to_string(),
            announcement,
            nodes,
        });
        Some(update)
    }
}

/// The layouts of `tree`'s text inputs that have one, which only the
/// presenter's elements hold.
pub(super) fn text_geometry<E: Element>(
    presenter: &Presenter<E>,
    tree: &AccessibilityTree,
) -> TextGeometry {
    fn collect<E: Element>(
        presenter: &Presenter<E>,
        node: &AccessibilityNode,
        text: &mut TextGeometry,
    ) {
        if node.a11y_role() == AccessibilityRole::TextInput
            && let Some(geometry) = presenter
                .root()
                .text_input_snapshot(node.a11y_id())
                .and_then(|snapshot| snapshot.geometry().cloned())
        {
            text.insert(node.a11y_id(), geometry);
        }
        for child in node.a11y_children() {
            collect(presenter, child, text);
        }
    }
    let mut text = TextGeometry::new();
    for root in tree.roots() {
        collect(presenter, root, &mut text);
    }
    text
}
