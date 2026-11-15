//! Pinyin scenarios: synthesized keys through rui's `keyDown:` into the real
//! input method, checked against the NSTextInputClient calls rui logged and
//! the value rui reported.

use crate::native::{self, Key};
use crate::probe::{Step, log_len, log_since, note, step};
use crate::{tis, view};
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_foundation::{NSNotFound, NSRange, NSString};

/// Apple Simplified Chinese Pinyin; `RUI_PROBE_INPUT_SOURCE` picks another.
pub const PINYIN: &str = "com.apple.inputmethod.SCIM.ITABC";

fn input_source() -> String {
    std::env::var("RUI_PROBE_INPUT_SOURCE").unwrap_or_else(|_| PINYIN.to_string())
}
const KEY_DELAY: u64 = 140;
const SETTLE: u64 = 500;

fn marked(log: &[String]) -> Vec<&String> {
    log.iter()
        .filter(|line| line.contains("setMarkedText"))
        .collect()
}

fn inserted(log: &[String]) -> Vec<&String> {
    log.iter()
        .filter(|line| line.contains("insertText"))
        .collect()
}

/// The quoted text of a logged `setMarkedText "…" selected=…` line.
fn preedit(line: &str) -> &str {
    let start = line.find('"').map_or(0, |at| at + 1);
    let end = line.rfind("\" selected=").unwrap_or(line.len());
    line.get(start..end).unwrap_or_default()
}

/// Whether some setMarkedText carried a Latin pinyin preedit.
fn has_pinyin_preedit(log: &[String]) -> bool {
    marked(log)
        .iter()
        .any(|line| preedit(line).chars().any(|ch| ch.is_ascii_lowercase()))
}

fn summary(log: &[String], value: &str) -> String {
    let unreached = if marked(log).is_empty() {
        " (the input method never composed: no setMarkedText)"
    } else {
        ""
    };
    format!(
        "value={value:?}, {} setMarkedText, {} insertText{unreached}",
        marked(log).len(),
        inserted(log).len()
    )
}

pub fn steps() -> Vec<Step> {
    let mut steps = vec![activate(), select_pinyin(), verify_pinyin()];
    // nihao, candidate window, then 1 picks the first candidate.
    steps.extend(scenario(view::COMMIT, |steps| {
        steps.push(context_source());
        steps.extend(keys("nihao"));
        steps.push(candidate_rect(view::COMMIT));
        steps.push(key(native::ONE));
    }));
    steps.push(check(view::COMMIT, |log, value| {
        let ok = has_pinyin_preedit(log)
            && inserted(log).iter().any(|line| line.contains("\"你好\""))
            && value == "你好";
        (ok, summary(log, value))
    }));
    if !native::hid_events() {
        steps.push(handle_event_scenario());
    }

    steps.extend(scenario(view::ESCAPE, |steps| {
        steps.extend(keys("nihao"));
        steps.push(key(native::ESCAPE));
    }));
    steps.push(check(view::ESCAPE, |log, value| {
        (
            !marked(log).is_empty() && value.is_empty(),
            summary(log, value),
        )
    }));

    // nix, Backspace edits the preedit back to ni, then hao and Space.
    steps.extend(scenario(view::BACKSPACE, |steps| {
        steps.extend(keys("nix"));
        steps.push(key(native::BACKSPACE));
        steps.extend(keys("hao"));
        steps.push(key(native::SPACE));
    }));
    steps.push(check(view::BACKSPACE, |log, value| {
        let marks = marked(log);
        let x_then_gone = marks
            .iter()
            .position(|line| preedit(line).contains('x'))
            .is_some_and(|at| {
                marks[at + 1..]
                    .iter()
                    .any(|line| !preedit(line).contains('x'))
            });
        (x_then_gone && value == "你好", summary(log, value))
    }));

    steps.extend(scenario(view::WORDS, |steps| {
        for word in ["nihao", "shijie", "zhongguo"] {
            steps.extend(keys(word));
            steps.push(key(native::SPACE));
        }
    }));
    steps.push(check(view::WORDS, |log, value| {
        (value == "你好世界中国", summary(log, value))
    }));

    steps.extend(scenario(view::NOTES, |steps| {
        steps.extend(keys("nihao"));
        steps.push(key(native::SPACE));
        steps.push(key(native::RETURN));
        steps.extend(keys("shijie"));
        steps.push(key(native::SPACE));
    }));
    steps.push(check(view::NOTES, |log, value| {
        (value == "你好\n世界", summary(log, value))
    }));

    // Shift alone mid-composition. Whether it switches Pinyin to English
    // depends on the user's Pinyin settings (Caps Lock may do that instead):
    // switched, the letters commit as typed; not switched, composition keeps
    // going and Escape clears it. Either way Shift must not break the preedit.
    steps.extend(scenario(view::SHIFT, |steps| {
        steps.extend(keys("nihao"));
        steps.push(shift());
        steps.extend(keys("ok"));
        steps.push(shift());
        steps.push(key(native::ESCAPE));
    }));
    steps.push(check(view::SHIFT, |log, value| {
        let marks = marked(log);
        let switched = value == "nihaook";
        let kept_composing =
            value.is_empty() && marks.iter().any(|line| preedit(line).contains("ok"));
        (
            !marks.is_empty() && (switched || kept_composing),
            summary(log, value),
        )
    }));

    steps.extend(scenario(view::EMULATED, emulated_input_method));
    steps.push(check(view::EMULATED, |log, value| {
        (value == "你好", summary(log, value))
    }));

    steps.push(step(|_| {
        tis::restore();
        200
    }));
    steps
}

fn activate() -> Step {
    step(|probe| {
        let app = native::app();
        #[allow(deprecated)]
        app.activateIgnoringOtherApps(true);
        probe.window.makeKeyAndOrderFront(None);
        let window = probe.window.clone();
        probe.push_front(vec![step(move |probe| {
            let active = native::app().isActive();
            let key = window.isKeyWindow();
            probe.record(
                "window becomes key in an active app",
                active && key,
                format!("NSApp.isActive={active} window.isKeyWindow={key}"),
            );
            if native::hid_events() {
                probe.record(
                    "probe may post HID keystrokes (Accessibility)",
                    crate::cg::trusted(),
                    format!("AXIsProcessTrusted and CGPreflightPostEventAccess={}", crate::cg::trusted()),
                );
            }
            0
        })]);
        600
    })
}

fn select_pinyin() -> Step {
    step(|_| {
        let source = input_source();
        let status = tis::select(&source);
        note(format!(
            "probe: TISSelectInputSource({source}) -> {status:?}"
        ));
        800
    })
}

/// The focused view's NSTextInputContext keeps its own keyboard input
/// source; report it and whether it is the current context.
fn context_source() -> Step {
    step(|probe| {
        let Some(view) = native::content_view(&probe.window) else {
            return 0;
        };
        let context: Option<Retained<AnyObject>> = unsafe { msg_send![&*view, inputContext] };
        let Some(context) = context else {
            probe.record(
                "view has an NSTextInputContext",
                false,
                "inputContext is nil",
            );
            return 0;
        };
        let current: Option<Retained<AnyObject>> =
            unsafe { msg_send![class!(NSTextInputContext), currentInputContext] };
        let selected: Option<Retained<NSString>> =
            unsafe { msg_send![&*context, selectedKeyboardInputSource] };
        let is_current = current
            .as_ref()
            .is_some_and(|current| Retained::as_ptr(current) == Retained::as_ptr(&context));
        note(format!(
            "probe: inputContext.selectedKeyboardInputSource={:?} isCurrentInputContext={is_current}",
            selected.map(|source| source.to_string())
        ));
        0
    })
}

fn verify_pinyin() -> Step {
    step(|probe| {
        let current = tis::current_id();
        probe.record(
            "Pinyin input source selected",
            current == Some(input_source()),
            format!("current input source {current:?}"),
        );
        0
    })
}

/// Click the field, then mark where its scenario's log starts and run `body`.
fn scenario(field: &'static str, body: impl FnOnce(&mut Vec<Step>)) -> Vec<Step> {
    let mut steps = vec![
        focus(field),
        step(move |probe| {
            probe.mark = log_len();
            let front: Option<Retained<AnyObject>> = unsafe {
                let workspace: Retained<AnyObject> =
                    msg_send![class!(NSWorkspace), sharedWorkspace];
                msg_send![&*workspace, frontmostApplication]
            };
            let front: Option<Retained<NSString>> =
                front.and_then(|app| unsafe { msg_send![&*app, bundleIdentifier] });
            note(format!(
                "---- scenario {field} active={} key={} front={:?}",
                native::app().isActive(),
                probe.window.isKeyWindow(),
                front.map(|id| id.to_string())
            ));
            0
        }),
    ];
    body(&mut steps);
    steps
}

pub fn focus(label: &'static str) -> Step {
    step(move |probe| {
        match crate::ax::find_by_label(&probe.window, label) {
            Some(element) => native::post_click(&probe.window, native::ax_frame(&element)),
            None => probe.record(
                format!("{label}: focus"),
                false,
                "no AX element with that label",
            ),
        }
        400
    })
}

fn keys(text: &'static str) -> Vec<Step> {
    text.chars().map(|ch| key(Key::letter(ch))).collect()
}

fn key(key: Key) -> Step {
    step(move |probe| {
        if probe.direct_keys {
            let handled = native::handle_key_in_context(&probe.window, key);
            note(format!("probe: handleEvent {:?} -> {handled}", key.chars));
            // The key up is a targeted event, so rui drains what the input
            // method sent.
            native::post_key_up(&probe.window, key);
        } else {
            native::post_key(&probe.window, key);
        }
        KEY_DELAY
    })
}

fn shift() -> Step {
    step(|probe| {
        native::post_shift_tap(&probe.window);
        KEY_DELAY * 2
    })
}

fn check(field: &'static str, verdict: fn(&[String], &str) -> (bool, String)) -> Step {
    step(move |probe| {
        // Let the last key's frame land first.
        probe.push_front(vec![step(move |probe| {
            let log = log_since(probe.mark);
            let value = view::value(field);
            let (pass, detail) = verdict(&log, &value);
            probe.record(format!("IME {field}"), pass, detail);
            0
        })]);
        SETTLE
    })
}

/// The candidate window rect: `firstRectForCharacterRange:` for the marked
/// text, which must sit on the field's line in screen coordinates.
fn candidate_rect(field: &'static str) -> Step {
    step(move |probe| {
        let asked_by_ime = log_since(probe.mark)
            .iter()
            .any(|line| line.contains("firstRectForCharacterRange"));
        let Some(view) = native::content_view(&probe.window) else {
            probe.record("candidate rect", false, "no content view");
            return 0;
        };
        let marked = native::marked_range(&view);
        note("probe: calling firstRectForCharacterRange itself".to_string());
        let (rect, actual) = native::first_rect(&view, marked);
        let caret = NSRange::new(marked.location.wrapping_add(marked.length), 0);
        let (caret_rect, _) = native::first_rect(&view, caret);
        let selected: NSRange = unsafe { msg_send![&*view, selectedRange] };
        let (selected_rect, _) = native::first_rect(&view, selected);
        note(format!(
            "probe: selectedRange={selected:?} firstRect={}",
            native::fmt_rect(selected_rect)
        ));
        let substring: Option<Retained<AnyObject>> = unsafe {
            msg_send![&*view, attributedSubstringForProposedRange: marked, actualRange: std::ptr::null_mut::<NSRange>()]
        };
        note(format!(
            "probe: caret firstRect={} attributedSubstring(markedRange)={:?}",
            native::fmt_rect(caret_rect),
            substring.map(|text| native::describe(&text))
        ));
        let field_frame = crate::ax::find_by_label(&probe.window, field)
            .map(|element| native::ax_frame(&element))
            .unwrap_or_else(native::empty_rect);
        let pass = marked.location != NSNotFound as usize
            && rect.size.height > 0.0
            && native::contains(field_frame, rect, 2.0);
        probe.record(
            format!("{field}: candidate rect on the field (screen coordinates)"),
            pass,
            format!(
                "markedRange={marked:?} firstRect={} actual={actual:?} field AXFrame={} \
                 window frame={} IME asked itself={asked_by_ime}",
                native::fmt_rect(rect),
                native::fmt_rect(field_frame),
                native::fmt_rect(probe.window.frame()),
            ),
        );
        0
    })
}

/// Retry the commit scenario through
/// `NSTextInputContext handleEvent:` directly, to tell rui's `keyDown:` path
/// apart from the input context.
fn handle_event_scenario() -> Step {
    step(|probe| {
        let reached = log_since(0)
            .iter()
            .any(|line| line.contains("setMarkedText"));
        note(format!(
            "probe: queued keys reached the input method: {reached}"
        ));
        let mut steps = scenario(view::DIRECT, |steps| {
            steps.push(step(|probe| {
                probe.direct_keys = true;
                0
            }));
            steps.extend(keys("nihao"));
            steps.push(key(native::SPACE));
            steps.push(step(|probe| {
                probe.direct_keys = false;
                0
            }));
        });
        steps.push(check(view::DIRECT, |log, value| {
            (value == "你好", summary(log, value))
        }));
        probe.push_front(steps);
        0
    })
}

/// One NSTextInputClient call made the way an input method makes it: from
/// the run loop, with no NSEvent after it (as when a candidate is clicked in
/// the input method's window).
enum ClientCall {
    Mark(&'static str),
    Insert(&'static str),
    Unmark,
}

fn client_call(call: ClientCall) -> Step {
    step(move |probe| {
        let Some(view) = native::content_view(&probe.window) else {
            return 0;
        };
        let none = NSRange::new(NSNotFound as usize, 0);
        unsafe {
            match call {
                ClientCall::Mark(text) => {
                    let marked = NSString::from_str(text);
                    let end = NSRange::new(text.encode_utf16().count(), 0);
                    let _: () = msg_send![
                        &*view,
                        setMarkedText: &*marked,
                        selectedRange: end,
                        replacementRange: none
                    ];
                }
                ClientCall::Insert(text) => {
                    let text = NSString::from_str(text);
                    let _: () = msg_send![&*view, insertText: &*text, replacementRange: none];
                }
                ClientCall::Unmark => {
                    let _: () = msg_send![&*view, unmarkText];
                }
            }
        }
        KEY_DELAY
    })
}

/// The call sequence Apple's Pinyin makes for `nihao` then a pick, made by the
/// probe: rui's side of the protocol, the candidate rect and the wake for
/// commands that arrive outside a key event.
fn emulated_input_method(steps: &mut Vec<Step>) {
    for preedit in ["n", "ni", "ni h", "ni ha", "ni hao"] {
        steps.push(client_call(ClientCall::Mark(preedit)));
    }
    steps.push(candidate_rect(view::EMULATED));
    steps.push(client_call(ClientCall::Insert("你好")));
    // A second composition cancelled the way Escape cancels it.
    steps.push(client_call(ClientCall::Mark("shi")));
    steps.push(client_call(ClientCall::Mark("")));
    steps.push(client_call(ClientCall::Unmark));
}
