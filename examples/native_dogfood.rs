//! Finite native macOS dogfood example for local GUI smoke testing.

use rui::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

const NATIVE_DOGFOOD_INPUT_ID: ElementId = ElementId(29_001);
const DEFAULT_DOGFOOD_TEXT: &str = "rui-native-dogfood";

fn main() {
    let config = NativeDogfoodConfig::load_from_environment();
    let options = WindowOptions::default()
        .title("RUI Native Dogfood")
        .size(520.0, 260.0)
        .resizable(false);

    App::new().run_view_with_options(options, NativeDogfoodView::new(config));
}

#[derive(Clone)]
struct NativeDogfoodConfig {
    expected_text: String,
    profile_path: Option<PathBuf>,
    interactive: bool,
}

impl NativeDogfoodConfig {
    fn load_from_environment() -> Self {
        Self {
            expected_text: std::env::var("RUI_NATIVE_DOGFOOD_TEXT")
                .unwrap_or_else(|_| String::from(DEFAULT_DOGFOOD_TEXT)),
            profile_path: std::env::var_os("RUI_PROFILE").map(PathBuf::from),
            interactive: std::env::var_os("RUI_NATIVE_DOGFOOD_INTERACTIVE").is_some(),
        }
    }
}

struct NativeDogfoodView {
    config: NativeDogfoodConfig,
    typed_text: Rc<RefCell<String>>,
    submitted_text: Rc<RefCell<Option<String>>>,
    profile_written: Rc<Cell<bool>>,
}

impl NativeDogfoodView {
    fn new(config: NativeDogfoodConfig) -> Self {
        Self {
            config,
            typed_text: Rc::new(RefCell::new(String::new())),
            submitted_text: Rc::new(RefCell::new(None)),
            profile_written: Rc::new(Cell::new(false)),
        }
    }

    fn write_profile(&self, status: &str, typed_text: &str, submitted: bool) {
        let Some(path) = &self.config.profile_path else {
            return;
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap_or_else(|err| {
                panic!(
                    "failed to create RUI_PROFILE directory {}: {err}",
                    parent.display()
                )
            });
        }

        let text_matched = typed_text == self.config.expected_text;
        let body = format!(
            "{{\"schema\":\"rui.native_dogfood.v1\",\"status\":\"{}\",\"example\":\"native_dogfood\",\"typed_text\":\"{}\",\"expected_text\":\"{}\",\"text_matched\":{},\"submitted\":{},\"interactive\":{},\"script_requires_minimize_reopen\":{},\"driver\":\"scripts/native_dogfood_macos.sh\"}}\n",
            json_escape(status),
            json_escape(typed_text),
            json_escape(&self.config.expected_text),
            text_matched,
            submitted,
            self.config.interactive,
            self.config.interactive,
        );
        std::fs::write(path, body)
            .unwrap_or_else(|err| panic!("failed to write RUI_PROFILE {}: {err}", path.display()));
    }
}

impl View for NativeDogfoodView {
    type Element = Div;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        if !self.config.interactive && !self.profile_written.get() {
            self.write_profile("finite_default_exit", "", false);
            self.profile_written.set(true);
            cx.app_mut().quit();
        }

        if let Some(submitted) = self.submitted_text.borrow().clone()
            && !self.profile_written.get()
        {
            let status = if submitted == self.config.expected_text {
                "passed"
            } else {
                "failed"
            };
            self.write_profile(status, &submitted, true);
            self.profile_written.set(true);
            cx.app_mut().quit();
        }

        let typed_text = self.typed_text.borrow().clone();
        let change_text = Rc::clone(&self.typed_text);
        let change_notifier = cx.notifier();
        let submit_text = Rc::clone(&self.submitted_text);
        let submit_notifier = cx.notifier();

        div()
            .w(520.0)
            .h(260.0)
            .bg(Color::hex(0xf8fafc))
            .flex_col()
            .p(24.0)
            .gap(14.0)
            .child(
                text("Native dogfood")
                    .size(26.0)
                    .bold()
                    .color(Color::hex(0x111827)),
            )
            .child(
                text(
                    "Click the input, type the dogfood text, minimize, reopen, then press Return.",
                )
                .size(13.0)
                .color(Color::hex(0x4b5563)),
            )
            .child(
                input()
                    .id(NATIVE_DOGFOOD_INPUT_ID)
                    .w(420.0)
                    .placeholder(DEFAULT_DOGFOOD_TEXT)
                    .accessibility_label("Native dogfood input")
                    .value(typed_text.clone())
                    .on_change(move |value| {
                        *change_text.borrow_mut() = String::from(value);
                        change_notifier.notify();
                    })
                    .on_submit(move |value| {
                        *submit_text.borrow_mut() = Some(String::from(value));
                        submit_notifier.notify();
                    }),
            )
            .child(
                text(format!(
                    "Captured input: {}",
                    native_dogfood_display_text(&typed_text)
                ))
                .size(13.0)
                .color(Color::hex(0x1f2937)),
            )
            .child(
                text("Profile output is written to RUI_PROFILE after submit.")
                    .size(12.0)
                    .color(Color::hex(0x6b7280)),
            )
    }
}

fn native_dogfood_display_text(value: &str) -> &str {
    if value.is_empty() { "(empty)" } else { value }
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped
}
