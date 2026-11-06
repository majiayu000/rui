//! The application menu bar and context menus, built as `NSMenu`s from the
//! menus the app declared with [`AppContext::set_menus`] and
//! [`AppContext::show_context_menu`].
//!
//! Every item targets one [`RuiMenuTarget`], which records the chosen item's
//! action; [`MacMenus::sync`] hands it to the app, and the key window routes it
//! in its next frame. An item shows the keymap shortcut of its action, but a
//! press of that shortcut reaches the keymap as a key event; the menu does not
//! run the action a second time.

use crate::core::action::{ActionId, KeyChord};
use crate::core::app::AppContext;
use crate::core::event::KeyCode;
use crate::core::menu::{ContextMenuRequest, Menu, MenuItem};
use crate::core::window::WindowId;
use crate::platform::mac::events::{MAC_REDRAW_EVENT_DATA, post_application_event};
use crate::platform::mac::window::MacWindow;
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyObject, NSObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, class, define_class, msg_send, sel};
use objc2_app_kit::{NSApplication, NSEventModifierFlags, NSEventType, NSView};
use objc2_foundation::{NSObjectProtocol, NSPoint, NSRect, NSString};
use std::cell::RefCell;

/// Window number of application events meant for no window: a wake.
const APP_EVENT_WINDOW_NUMBER: isize = 0;
const CONTROL_STATE_ON: isize = 1;
const CONTROL_STATE_OFF: isize = 0;

#[derive(Default)]
pub(crate) struct RuiMenuTargetIvars {
    /// The action of each item, by the item's tag.
    actions: RefCell<Vec<ActionId>>,
    chosen: RefCell<Vec<ActionId>>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements, the ivars are
    // main-thread-only, and this class does not implement Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "RuiMenuTarget"]
    #[ivars = RuiMenuTargetIvars]
    pub(crate) struct RuiMenuTarget;

    unsafe impl NSObjectProtocol for RuiMenuTarget {}

    impl RuiMenuTarget {
        #[unsafe(method(performMenuItem:))]
        fn perform_menu_item(&self, item: &AnyObject) {
            if chosen_by_its_key_equivalent(item) {
                return;
            }
            let tag: isize = unsafe { msg_send![item, tag] };
            let action = usize::try_from(tag)
                .ok()
                .and_then(|tag| self.ivars().actions.borrow().get(tag).cloned());
            let Some(action) = action else {
                log::error!("macOS menu item with tag {tag} has no action");
                return;
            };
            self.ivars().chosen.borrow_mut().push(action);
            if let Err(err) = post_application_event(APP_EVENT_WINDOW_NUMBER, MAC_REDRAW_EVENT_DATA)
            {
                log::error!("failed to wake the event loop for a menu item: {err}");
            }
        }
    }
);

impl RuiMenuTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(RuiMenuTargetIvars::default());
        unsafe { msg_send![super(this), init] }
    }
}

/// Whether AppKit runs `item` because its key equivalent was pressed. That
/// key press also reached the window, where the keymap runs the action.
fn chosen_by_its_key_equivalent(item: &AnyObject) -> bool {
    let mtm = MainThreadMarker::new().expect("menu items run on the main thread");
    let Some(event) = NSApplication::sharedApplication(mtm).currentEvent() else {
        return false;
    };
    if event.r#type() != NSEventType::KeyDown {
        return false;
    }
    let equivalent: Retained<NSString> = unsafe { msg_send![item, keyEquivalent] };
    let pressed = event
        .charactersIgnoringModifiers()
        .map(|chars| chars.to_string().to_lowercase());
    !equivalent.is_empty() && pressed.as_deref() == Some(equivalent.to_string().as_str())
}

/// The app's native menus.
pub(crate) struct MacMenus {
    target: Retained<RuiMenuTarget>,
    /// Items of the menu bar; context menu items are tagged after them.
    bar_items: usize,
}

impl MacMenus {
    pub(crate) fn new(mtm: MainThreadMarker) -> Self {
        Self {
            target: RuiMenuTarget::new(mtm),
            bar_items: 0,
        }
    }

    /// Show the menu bar and the context menu the app asked for, and hand it
    /// the actions chosen from them. Without a key window to route them they
    /// go to the app's action handlers.
    pub(crate) fn sync<'a>(
        &mut self,
        app: &NSApplication,
        context: &mut AppContext,
        windows: impl Iterator<Item = (WindowId, &'a MacWindow)> + Clone,
    ) {
        if context.take_menu_bar_update() {
            self.target.ivars().actions.borrow_mut().clear();
            let bar = ns_menu("");
            for menu in context.menus() {
                let item = new_item(menu.title(), "");
                let submenu = self.build(menu, context);
                let _: () = unsafe { msg_send![&*item, setSubmenu: &*submenu] };
                let _: () = unsafe { msg_send![&*bar, addItem: &*item] };
            }
            let _: () = unsafe { msg_send![app, setMainMenu: &*bar] };
            self.bar_items = self.target.ivars().actions.borrow().len();
        }
        if let Some(request) = context.take_context_menu() {
            self.pop_up(request, context, windows.clone());
        }
        let chosen = std::mem::take(&mut *self.target.ivars().chosen.borrow_mut());
        if chosen.is_empty() {
            return;
        }
        // As AppKit sends menu actions: to the key window, else the main one.
        let target = windows
            .clone()
            .find(|(_, window)| window.is_focused())
            .or_else(|| windows.clone().next())
            .map(|(id, _)| id);
        // Queued on that window's frame state, so it draws next and routes them.
        let routed = target.is_some_and(|id| context.activate_window(id));
        for action in chosen {
            if routed {
                context.queue_menu_action(action);
            } else if !context.dispatch_app_action(&action).is_handled() {
                log::error!("menu action {action:?} had no handler and no window is open");
            }
        }
    }

    fn pop_up<'a>(
        &self,
        request: ContextMenuRequest,
        context: &AppContext,
        mut windows: impl Iterator<Item = (WindowId, &'a MacWindow)>,
    ) {
        let window = windows.find(|(id, _)| Some(*id) == request.window);
        let Some((_, window)) = window else {
            log::error!("context menu asked for a window that is not open");
            return;
        };
        let view: &NSView = &window.content_view;
        let menu = self.build(&request.menu, context);
        let bounds: NSRect = view.bounds();
        // The content view is not flipped: its origin is at the bottom left.
        let location = NSPoint::new(
            f64::from(request.position.x),
            bounds.size.height - f64::from(request.position.y),
        );
        // Tracks the menu until an item is chosen or it is dismissed.
        let _: bool = unsafe {
            msg_send![&*menu, popUpMenuPositioningItem: Option::<&AnyObject>::None, atLocation: location, inView: view]
        };
        self.target
            .ivars()
            .actions
            .borrow_mut()
            .truncate(self.bar_items);
    }

    fn build(&self, menu: &Menu, context: &AppContext) -> Retained<AnyObject> {
        let ns = ns_menu(menu.title());
        for item in menu.items() {
            let ns_item = match item {
                MenuItem::Separator => unsafe { msg_send![class!(NSMenuItem), separatorItem] },
                MenuItem::Submenu(submenu) => {
                    let ns_item = new_item(submenu.title(), "");
                    let built = self.build(submenu, context);
                    let _: () = unsafe { msg_send![&*ns_item, setSubmenu: &*built] };
                    ns_item
                }
                MenuItem::Action(action) => {
                    let chord = context.keymap().chord_for_action(action.action());
                    let (equivalent, modifiers) = chord
                        .and_then(key_equivalent)
                        .unwrap_or((String::new(), NSEventModifierFlags::empty()));
                    let ns_item = new_item(action.label(), &equivalent);
                    let mut actions = self.target.ivars().actions.borrow_mut();
                    let tag = actions.len() as isize;
                    actions.push(action.action().clone());
                    let state = if action.is_checked() {
                        CONTROL_STATE_ON
                    } else {
                        CONTROL_STATE_OFF
                    };
                    unsafe {
                        let _: () = msg_send![&*ns_item, setTarget: &*self.target];
                        let _: () = msg_send![&*ns_item, setTag: tag];
                        let _: () = msg_send![&*ns_item, setEnabled: action.is_enabled()];
                        let _: () = msg_send![&*ns_item, setState: state];
                        let _: () = msg_send![&*ns_item, setKeyEquivalentModifierMask: modifiers];
                    }
                    ns_item
                }
            };
            let _: () = unsafe { msg_send![&*ns, addItem: &*ns_item] };
        }
        ns
    }
}

fn ns_menu(title: &str) -> Retained<AnyObject> {
    let title = NSString::from_str(title);
    let menu: Allocated<AnyObject> = unsafe { msg_send![class!(NSMenu), alloc] };
    let menu: Retained<AnyObject> = unsafe { msg_send![menu, initWithTitle: &*title] };
    // Items are enabled as the app declared, not by AppKit's validation.
    let _: () = unsafe { msg_send![&*menu, setAutoenablesItems: false] };
    menu
}

fn new_item(title: &str, key_equivalent: &str) -> Retained<AnyObject> {
    let title = NSString::from_str(title);
    let key_equivalent = NSString::from_str(key_equivalent);
    let item: Allocated<AnyObject> = unsafe { msg_send![class!(NSMenuItem), alloc] };
    unsafe {
        msg_send![item, initWithTitle: &*title, action: Some(sel!(performMenuItem:)), keyEquivalent: &*key_equivalent]
    }
}

/// The `NSMenuItem` key equivalent and modifier mask showing `chord`, if
/// AppKit can show its key.
pub(crate) fn key_equivalent(chord: KeyChord) -> Option<(String, NSEventModifierFlags)> {
    let key = match chord.key() {
        KeyCode::A => 'a',
        KeyCode::B => 'b',
        KeyCode::C => 'c',
        KeyCode::D => 'd',
        KeyCode::E => 'e',
        KeyCode::F => 'f',
        KeyCode::G => 'g',
        KeyCode::H => 'h',
        KeyCode::I => 'i',
        KeyCode::J => 'j',
        KeyCode::K => 'k',
        KeyCode::L => 'l',
        KeyCode::M => 'm',
        KeyCode::N => 'n',
        KeyCode::O => 'o',
        KeyCode::P => 'p',
        KeyCode::Q => 'q',
        KeyCode::R => 'r',
        KeyCode::S => 's',
        KeyCode::T => 't',
        KeyCode::U => 'u',
        KeyCode::V => 'v',
        KeyCode::W => 'w',
        KeyCode::X => 'x',
        KeyCode::Y => 'y',
        KeyCode::Z => 'z',
        KeyCode::Key0 => '0',
        KeyCode::Key1 => '1',
        KeyCode::Key2 => '2',
        KeyCode::Key3 => '3',
        KeyCode::Key4 => '4',
        KeyCode::Key5 => '5',
        KeyCode::Key6 => '6',
        KeyCode::Key7 => '7',
        KeyCode::Key8 => '8',
        KeyCode::Key9 => '9',
        KeyCode::Minus => '-',
        KeyCode::Equal => '=',
        KeyCode::BracketLeft => '[',
        KeyCode::BracketRight => ']',
        KeyCode::Backslash => '\\',
        KeyCode::Semicolon => ';',
        KeyCode::Quote => '\'',
        KeyCode::Comma => ',',
        KeyCode::Period => '.',
        KeyCode::Slash => '/',
        KeyCode::Backquote => '`',
        KeyCode::Space => ' ',
        KeyCode::Enter => '\r',
        KeyCode::Tab => '\t',
        KeyCode::Escape => '\u{1b}',
        // The Mac keyboard's delete key, and forward delete.
        KeyCode::Backspace => '\u{7f}',
        KeyCode::Delete => '\u{f728}',
        KeyCode::ArrowUp => '\u{f700}',
        KeyCode::ArrowDown => '\u{f701}',
        KeyCode::ArrowLeft => '\u{f702}',
        KeyCode::ArrowRight => '\u{f703}',
        KeyCode::Home => '\u{f729}',
        KeyCode::End => '\u{f72b}',
        KeyCode::PageUp => '\u{f72c}',
        KeyCode::PageDown => '\u{f72d}',
        key if key.is_function_key() => {
            let index = function_key_index(key)?;
            char::from_u32(0xf704 + index)?
        }
        _ => return None,
    };
    let modifiers = chord.modifiers();
    let mut mask = NSEventModifierFlags::empty();
    if modifiers.shift {
        mask |= NSEventModifierFlags::Shift;
    }
    if modifiers.ctrl {
        mask |= NSEventModifierFlags::Control;
    }
    if modifiers.alt {
        mask |= NSEventModifierFlags::Option;
    }
    if modifiers.meta {
        mask |= NSEventModifierFlags::Command;
    }
    Some((key.to_string(), mask))
}

fn function_key_index(key: KeyCode) -> Option<u32> {
    const KEYS: [KeyCode; 20] = [
        KeyCode::F1,
        KeyCode::F2,
        KeyCode::F3,
        KeyCode::F4,
        KeyCode::F5,
        KeyCode::F6,
        KeyCode::F7,
        KeyCode::F8,
        KeyCode::F9,
        KeyCode::F10,
        KeyCode::F11,
        KeyCode::F12,
        KeyCode::F13,
        KeyCode::F14,
        KeyCode::F15,
        KeyCode::F16,
        KeyCode::F17,
        KeyCode::F18,
        KeyCode::F19,
        KeyCode::F20,
    ];
    KEYS.iter()
        .position(|candidate| *candidate == key)
        .map(|index| index as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::event::Modifiers;

    #[test]
    fn key_equivalents_show_the_chord_key_and_its_modifiers() {
        let copy = key_equivalent(KeyChord::new(KeyCode::C, Modifiers::meta()));
        assert_eq!(copy, Some(("c".to_owned(), NSEventModifierFlags::Command)));

        let mut shifted = Modifiers::meta();
        shifted.shift = true;
        let redo = key_equivalent(KeyChord::new(KeyCode::Z, shifted));
        assert_eq!(
            redo,
            Some((
                "z".to_owned(),
                NSEventModifierFlags::Command | NSEventModifierFlags::Shift
            ))
        );

        let f5 = key_equivalent(KeyChord::new(KeyCode::F5, Modifiers::none()));
        assert_eq!(
            f5,
            Some(("\u{f708}".to_owned(), NSEventModifierFlags::empty()))
        );
        assert_eq!(
            key_equivalent(KeyChord::new(KeyCode::Meta, Modifiers::none())),
            None
        );
    }
}
