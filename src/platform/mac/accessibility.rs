use crate::core::ElementId;
use crate::core::accessibility::{
    AccessibilityAction, AccessibilityAnnouncement, AccessibilityAnnouncementKind,
    AccessibilityBridge, AccessibilityError, AccessibilityNode, AccessibilityRole,
    AccessibilityTree,
};
use crate::core::geometry::Bounds;
use crate::platform::mac::events::{MAC_ACCESSIBILITY_EVENT_DATA, post_application_event};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Sel};
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSAccessibility, NSAccessibilityActionName, NSAccessibilityAnnouncementKey,
    NSAccessibilityAnnouncementRequestedNotification, NSAccessibilityButtonRole,
    NSAccessibilityCellRole, NSAccessibilityCheckBoxRole, NSAccessibilityDecrementAction,
    NSAccessibilityDialogSubrole, NSAccessibilityElement,
    NSAccessibilityFocusedUIElementChangedNotification, NSAccessibilityGroupRole,
    NSAccessibilityIncrementAction, NSAccessibilityLayoutChangedNotification,
    NSAccessibilityListRole, NSAccessibilityMenuItemRole, NSAccessibilityMenuRole,
    NSAccessibilityOutlineRole, NSAccessibilityPostNotification,
    NSAccessibilityPostNotificationWithUserInfo, NSAccessibilityPressAction,
    NSAccessibilityPriorityKey, NSAccessibilityPriorityLevel, NSAccessibilityProgressIndicatorRole,
    NSAccessibilityRadioButtonRole, NSAccessibilityRole, NSAccessibilityRowRole,
    NSAccessibilityScrollAreaRole, NSAccessibilitySelectedTextChangedNotification,
    NSAccessibilityStaticTextRole, NSAccessibilityTabGroupRole, NSAccessibilityTextFieldRole,
    NSAccessibilityTitleChangedNotification, NSAccessibilityToolbarRole,
    NSAccessibilityValueChangedNotification, NSView,
};
use objc2_foundation::{
    MainThreadMarker, NSArray, NSDictionary, NSNumber, NSObjectProtocol, NSPoint, NSRange, NSRect,
    NSSize, NSString,
};
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MacAccessibilityActionRequest {
    pub id: ElementId,
    pub request: MacAccessibilityRequest,
    pub bounds: Bounds,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum MacAccessibilityRequest {
    Action {
        action: AccessibilityAction,
        value: Option<String>,
    },
    Focus(bool),
}

struct MacAccessibilityElementIvars {
    id: ElementId,
    actions: RefCell<Vec<AccessibilityAction>>,
    bounds: Cell<Bounds>,
    requests: Rc<RefCell<VecDeque<MacAccessibilityActionRequest>>>,
    window_number: isize,
}

define_class!(
    // SAFETY: NSAccessibilityElement supports subclassing and all ivars stay
    // on AppKit's main thread. The class has no custom Drop implementation.
    #[unsafe(super = NSAccessibilityElement)]
    #[thread_kind = MainThreadOnly]
    #[ivars = MacAccessibilityElementIvars]
    struct RuiAccessibilityElement;

    impl RuiAccessibilityElement {
        #[unsafe(method(accessibilityPerformPress))]
        fn accessibility_perform_press(&self) -> bool {
            self.enqueue_action(AccessibilityAction::Activate, None)
        }

        #[unsafe(method(accessibilityPerformIncrement))]
        fn accessibility_perform_increment(&self) -> bool {
            self.enqueue_action(AccessibilityAction::ScrollForward, None)
        }

        #[unsafe(method(accessibilityPerformDecrement))]
        fn accessibility_perform_decrement(&self) -> bool {
            self.enqueue_action(AccessibilityAction::ScrollBackward, None)
        }

        #[unsafe(method(accessibilityPerformAction:))]
        fn accessibility_perform_action(&self, action: &NSAccessibilityActionName) {
            unsafe {
                if action == NSAccessibilityPressAction {
                    self.enqueue_action(AccessibilityAction::Activate, None);
                } else if action == NSAccessibilityDecrementAction {
                    self.enqueue_action(AccessibilityAction::ScrollBackward, None);
                } else if action == NSAccessibilityIncrementAction {
                    self.enqueue_action(AccessibilityAction::ScrollForward, None);
                }
            }
        }

        #[unsafe(method(setAccessibilityValue:))]
        fn set_accessibility_value(&self, value: Option<&AnyObject>) {
            if !self.supports(AccessibilityAction::SetValue) {
                return;
            }
            let Some(value) = value.and_then(|value| value.downcast_ref::<NSString>()) else {
                log::error!("macOS accessibility value setter received a non-string value");
                return;
            };
            self.enqueue_action(AccessibilityAction::SetValue, Some(value.to_string()));
        }

        #[unsafe(method(setAccessibilityFocused:))]
        fn set_accessibility_focused(&self, focused: bool) {
            self.enqueue_request(MacAccessibilityRequest::Focus(focused));
        }

        #[unsafe(method(isAccessibilitySelectorAllowed:))]
        unsafe fn is_accessibility_selector_allowed(&self, selector: Sel) -> bool {
            if selector == sel!(accessibilityPerformAction:) {
                return self
                    .ivars()
                    .actions
                    .borrow()
                    .iter()
                    .any(|action| *action != AccessibilityAction::SetValue)
                    .into();
            }
            if selector == sel!(setAccessibilityFocused:) {
                return true.into();
            }
            if selector == sel!(setAccessibilitySelectedText:)
                || selector == sel!(setAccessibilitySelectedTextRange:)
            {
                return false.into();
            }
            if let Some(action) = action_for_selector(selector) {
                return self.supports(action).into();
            }
            unsafe { msg_send![super(self), isAccessibilitySelectorAllowed: selector] }
        }
    }

    // SAFETY: NSObjectProtocol has no additional safety requirements.
    unsafe impl NSObjectProtocol for RuiAccessibilityElement {}
);

impl RuiAccessibilityElement {
    fn new(
        id: ElementId,
        actions: &[AccessibilityAction],
        bounds: Bounds,
        requests: Rc<RefCell<VecDeque<MacAccessibilityActionRequest>>>,
        window_number: isize,
        mtm: MainThreadMarker,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(MacAccessibilityElementIvars {
            id,
            actions: RefCell::new(actions.to_vec()),
            bounds: Cell::new(bounds),
            requests,
            window_number,
        });
        unsafe { msg_send![super(this), init] }
    }

    fn update_semantics(&self, actions: &[AccessibilityAction], bounds: Bounds) {
        *self.ivars().actions.borrow_mut() = actions.to_vec();
        self.ivars().bounds.set(bounds);
    }

    fn supports(&self, action: AccessibilityAction) -> bool {
        self.ivars().actions.borrow().contains(&action)
    }

    fn enqueue_action(&self, action: AccessibilityAction, value: Option<String>) -> bool {
        if !self.supports(action) {
            return false;
        }
        self.enqueue_request(MacAccessibilityRequest::Action { action, value })
    }

    fn enqueue_request(&self, request: MacAccessibilityRequest) -> bool {
        self.ivars()
            .requests
            .borrow_mut()
            .push_back(MacAccessibilityActionRequest {
                id: self.ivars().id,
                request,
                bounds: self.ivars().bounds.get(),
            });
        if let Err(err) =
            post_application_event(self.ivars().window_number, MAC_ACCESSIBILITY_EVENT_DATA)
        {
            self.ivars().requests.borrow_mut().pop_back();
            log::error!("failed to wake macOS event loop for accessibility action: {err}");
            return false;
        }
        true
    }

    fn set_native_value(&self, value: Option<&AnyObject>) {
        unsafe {
            let _: () = msg_send![super(self), setAccessibilityValue: value];
        }
    }

    fn set_native_focused(&self, focused: bool) {
        unsafe {
            let _: () = msg_send![super(self), setAccessibilityFocused: focused];
        }
    }

    fn invalidate(&self) {
        self.ivars().actions.borrow_mut().clear();
        unsafe {
            self.setAccessibilityParent(None);
            self.setAccessibilityChildren(None);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MacAccessibilitySnapshot {
    nodes: Vec<MacAccessibilityNodeSnapshot>,
}

impl MacAccessibilitySnapshot {
    pub fn nodes(&self) -> &[MacAccessibilityNodeSnapshot] {
        &self.nodes
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MacAccessibilityNodeSnapshot {
    id: ElementId,
    native_role: &'static str,
    label: Option<String>,
    value: Option<String>,
    enabled: bool,
    read_only: bool,
    invalid: bool,
    focused: bool,
    selected: Option<bool>,
    checked: Option<bool>,
    native_actions: Vec<&'static str>,
    children: Vec<MacAccessibilityNodeSnapshot>,
}

impl MacAccessibilityNodeSnapshot {
    pub fn id(&self) -> ElementId {
        self.id
    }

    pub fn native_role(&self) -> &'static str {
        self.native_role
    }

    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    pub fn value(&self) -> Option<&str> {
        self.value.as_deref()
    }

    pub fn enabled(&self) -> bool {
        self.enabled
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }

    pub fn invalid(&self) -> bool {
        self.invalid
    }

    pub fn focused(&self) -> bool {
        self.focused
    }

    pub fn selected(&self) -> Option<bool> {
        self.selected
    }

    pub fn checked(&self) -> Option<bool> {
        self.checked
    }

    pub fn native_actions(&self) -> &[&'static str] {
        &self.native_actions
    }

    pub fn children(&self) -> &[MacAccessibilityNodeSnapshot] {
        &self.children
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MacAccessibilityAnnouncementSnapshot {
    node_id: ElementId,
    native_notification: &'static str,
    message: String,
}

impl MacAccessibilityAnnouncementSnapshot {
    pub fn node_id(&self) -> ElementId {
        self.node_id
    }

    pub fn native_notification(&self) -> &'static str {
        self.native_notification
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

pub struct MacAccessibilityBridge {
    native_host: Option<Box<dyn NativeAccessibilityHost>>,
}

impl fmt::Debug for MacAccessibilityBridge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MacAccessibilityBridge")
            .field("native_attached", &self.native_attached())
            .finish()
    }
}

impl Default for MacAccessibilityBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl MacAccessibilityBridge {
    pub fn new() -> Self {
        Self { native_host: None }
    }

    pub(crate) fn attached_to(content_view: Retained<NSView>, window_number: isize) -> Self {
        Self {
            native_host: Some(Box::new(AppKitAccessibilityHost::new(
                content_view,
                window_number,
            ))),
        }
    }

    pub fn native_attached(&self) -> bool {
        self.native_host.is_some()
    }

    pub(crate) fn take_action_request(&mut self) -> Option<MacAccessibilityActionRequest> {
        self.native_host
            .as_mut()
            .and_then(|host| host.take_action_request())
    }

    pub fn snapshot_tree(&self, tree: &AccessibilityTree) -> MacAccessibilitySnapshot {
        MacAccessibilitySnapshot {
            nodes: tree.roots().iter().map(snapshot_node).collect(),
        }
    }

    pub fn snapshot_announcement(
        &self,
        announcement: &AccessibilityAnnouncement,
    ) -> MacAccessibilityAnnouncementSnapshot {
        MacAccessibilityAnnouncementSnapshot {
            node_id: announcement.node_id(),
            native_notification: native_notification_name(announcement.kind()),
            message: announcement.message().to_string(),
        }
    }

    fn missing_native_bridge() -> AccessibilityError {
        AccessibilityError::BridgeFailure {
            message: "macOS accessibility bridge is not attached to a native AppKit host"
                .to_string(),
        }
    }

    #[cfg(test)]
    fn with_host(host: impl NativeAccessibilityHost + 'static) -> Self {
        Self {
            native_host: Some(Box::new(host)),
        }
    }
}

trait NativeAccessibilityHost {
    fn publish_tree(&mut self, tree: &AccessibilityTree) -> Result<(), AccessibilityError>;
    fn announce(
        &mut self,
        announcement: &AccessibilityAnnouncement,
    ) -> Result<(), AccessibilityError>;
    fn take_action_request(&mut self) -> Option<MacAccessibilityActionRequest>;
}

struct AppKitAccessibilityHost {
    content_view: Retained<NSView>,
    elements: HashMap<ElementId, Retained<RuiAccessibilityElement>>,
    action_requests: Rc<RefCell<VecDeque<MacAccessibilityActionRequest>>>,
    window_number: isize,
    last_tree: Option<AccessibilityTree>,
}

impl AppKitAccessibilityHost {
    fn new(content_view: Retained<NSView>, window_number: isize) -> Self {
        content_view.setAccessibilityElement(false);
        Self {
            content_view,
            elements: HashMap::new(),
            action_requests: Rc::new(RefCell::new(VecDeque::new())),
            window_number,
            last_tree: None,
        }
    }

    fn build_node(
        node: &AccessibilityNode,
        parent: &AnyObject,
        parent_bounds: Bounds,
        existing: &HashMap<ElementId, Retained<RuiAccessibilityElement>>,
        elements: &mut HashMap<ElementId, Retained<RuiAccessibilityElement>>,
        requests: &Rc<RefCell<VecDeque<MacAccessibilityActionRequest>>>,
        window_number: isize,
        mtm: MainThreadMarker,
    ) -> Result<Retained<RuiAccessibilityElement>, AccessibilityError> {
        let element = existing.get(&node.a11y_id()).cloned().unwrap_or_else(|| {
            RuiAccessibilityElement::new(
                node.a11y_id(),
                node.a11y_actions(),
                node.a11y_bounds().unwrap_or(parent_bounds),
                Rc::clone(requests),
                window_number,
                mtm,
            )
        });
        let bounds = node.a11y_bounds().unwrap_or(parent_bounds);
        element.update_semantics(node.a11y_actions(), bounds);
        element.setAccessibilityElement(true);
        element.setAccessibilityRole(Some(native_role(node.a11y_role())));
        element.setAccessibilitySubrole(
            (node.a11y_role() == AccessibilityRole::Dialog)
                .then_some(unsafe { NSAccessibilityDialogSubrole }),
        );
        element.setAccessibilityEnabled(node.a11y_enabled());
        element.set_native_focused(node.a11y_focused());
        element.setAccessibilityFrameInParentSpace(native_frame_in_parent(bounds, parent_bounds));
        let label = node.a11y_label().map(NSString::from_str);
        element.setAccessibilityLabel(label.as_deref());
        publish_native_value(&element, node)?;
        publish_text_semantics(&element, node)?;
        publish_value_description(&element, node);
        element.setAccessibilitySelected(node.a11y_selected().unwrap_or(false));
        unsafe {
            element.setAccessibilityParent(Some(parent));
        }

        let mut children: Vec<Retained<AnyObject>> = Vec::with_capacity(node.a11y_children().len());
        for child in node.a11y_children() {
            let parent: &AnyObject = &element;
            let child = Self::build_node(
                child,
                parent,
                bounds,
                existing,
                elements,
                requests,
                window_number,
                mtm,
            )?;
            children.push(unsafe { Retained::cast_unchecked(child) });
        }
        let native_children = NSArray::from_retained_slice(&children);
        unsafe {
            element.setAccessibilityChildren(Some(&native_children));
        }
        elements.insert(node.a11y_id(), element.clone());
        Ok(element)
    }
}

impl NativeAccessibilityHost for AppKitAccessibilityHost {
    fn publish_tree(&mut self, tree: &AccessibilityTree) -> Result<(), AccessibilityError> {
        let layout_changed = self
            .last_tree
            .as_ref()
            .is_none_or(|previous| accessibility_layout_changed(previous, tree));
        let existing = std::mem::take(&mut self.elements);
        let mut elements = HashMap::new();
        let mut roots: Vec<Retained<AnyObject>> = Vec::with_capacity(tree.roots().len());
        let view_bounds = self.content_view.bounds();
        let parent_bounds = Bounds::from_xywh(
            0.0,
            0.0,
            view_bounds.size.width as f32,
            view_bounds.size.height as f32,
        );
        for root in tree.roots() {
            let parent: &AnyObject = &self.content_view;
            let root = Self::build_node(
                root,
                parent,
                parent_bounds,
                &existing,
                &mut elements,
                &self.action_requests,
                self.window_number,
                self.content_view.mtm(),
            )?;
            roots.push(unsafe { Retained::cast_unchecked(root) });
        }
        for (id, element) in &existing {
            if !elements.contains_key(id) {
                element.invalidate();
            }
        }

        let native_roots = NSArray::from_retained_slice(&roots);
        unsafe {
            self.content_view
                .setAccessibilityChildren(Some(&native_roots));
            if layout_changed {
                let host: &AnyObject = &self.content_view;
                NSAccessibilityPostNotification(host, NSAccessibilityLayoutChangedNotification);
            }
        }

        if let Some(previous) = &self.last_tree {
            post_property_notifications(previous, tree, &elements);
        }

        self.elements = elements;
        self.last_tree = Some(tree.clone());
        Ok(())
    }

    fn announce(
        &mut self,
        announcement: &AccessibilityAnnouncement,
    ) -> Result<(), AccessibilityError> {
        unsafe {
            match announcement.kind() {
                AccessibilityAnnouncementKind::FocusChanged => {
                    let element = self.elements.get(&announcement.node_id()).ok_or_else(|| {
                        AccessibilityError::BridgeFailure {
                            message: format!(
                                "macOS accessibility focus target {:?} is not in the published tree",
                                announcement.node_id()
                            ),
                        }
                    })?;
                    let element: &AnyObject = element;
                    NSAccessibilityPostNotification(
                        element,
                        NSAccessibilityFocusedUIElementChangedNotification,
                    );
                }
                AccessibilityAnnouncementKind::ActionFeedback => {
                    let message = NSString::from_str(announcement.message());
                    let priority = NSNumber::new_i64(NSAccessibilityPriorityLevel::Medium.0 as i64);
                    let message: &AnyObject = &message;
                    let priority: &AnyObject = &priority;
                    let user_info = NSDictionary::from_slices(
                        &[NSAccessibilityAnnouncementKey, NSAccessibilityPriorityKey],
                        &[message, priority],
                    );
                    let host: &AnyObject = &self.content_view;
                    NSAccessibilityPostNotificationWithUserInfo(
                        host,
                        NSAccessibilityAnnouncementRequestedNotification,
                        Some(&user_info),
                    );
                }
            }
        }
        Ok(())
    }

    fn take_action_request(&mut self) -> Option<MacAccessibilityActionRequest> {
        let request = self.action_requests.borrow_mut().pop_front()?;
        if self.elements.contains_key(&request.id) {
            Some(request)
        } else {
            log::error!(
                "discarded stale macOS accessibility action for removed element {:?}",
                request.id
            );
            None
        }
    }
}

include!("accessibility_mapping.rs");

impl AccessibilityBridge for MacAccessibilityBridge {
    fn publish_tree(&mut self, tree: &AccessibilityTree) -> Result<(), AccessibilityError> {
        validate_tree(tree)?;
        match self.native_host.as_mut() {
            Some(host) => host.publish_tree(tree),
            None => Err(Self::missing_native_bridge()),
        }
    }

    fn announce(
        &mut self,
        announcement: &AccessibilityAnnouncement,
    ) -> Result<(), AccessibilityError> {
        match self.native_host.as_mut() {
            Some(host) => host.announce(announcement),
            None => Err(Self::missing_native_bridge()),
        }
    }
}

#[cfg(test)]
#[path = "accessibility_tests.rs"]
mod tests;
