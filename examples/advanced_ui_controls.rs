//! Advanced UI controls dogfood app backed by local repository data.

use rui::advanced_ui as ui;
use rui::prelude::*;
use std::cell::{Cell, RefCell};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

pub const DOGFOOD_REFRESH_BUTTON_ID: ElementId = ElementId(28_001);
pub const DOGFOOD_PANEL_CONTROL_ID: ElementId = ElementId(28_002);
pub const DOGFOOD_CLAIM_GATE_ID: ElementId = ElementId(28_003);
pub const DOGFOOD_ACTIVITY_SCROLL_ID: ElementId = ElementId(28_004);

fn main() {
    let finite = std::env::var_os("RUI_ADVANCED_UI_HOLD").is_none();
    App::new().run_view(DogfoodControlsView::new(finite));
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDogfoodData {
    pub package_name: String,
    pub package_version: String,
    pub advanced_ui_files: usize,
    pub integration_tests: usize,
    pub examples: usize,
    pub git_changes: Vec<String>,
    pub verification_checks: Vec<LocalVerificationCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalVerificationCheck {
    pub label: String,
    pub passed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DogfoodDataError {
    Io {
        path: PathBuf,
        message: String,
    },
    MissingPackageField {
        field: &'static str,
    },
    InvalidPackageField {
        field: &'static str,
    },
    Command {
        command: String,
        message: String,
    },
    Utf8 {
        source: &'static str,
        message: String,
    },
}

impl fmt::Display for DogfoodDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => {
                write!(f, "failed to read {}: {message}", path.display())
            }
            Self::MissingPackageField { field } => {
                write!(f, "Cargo.toml is missing package field `{field}`")
            }
            Self::InvalidPackageField { field } => {
                write!(
                    f,
                    "Cargo.toml package field `{field}` is not a quoted string"
                )
            }
            Self::Command { command, message } => write!(f, "`{command}` failed: {message}"),
            Self::Utf8 { source, message } => write!(f, "{source} was not valid UTF-8: {message}"),
        }
    }
}

impl std::error::Error for DogfoodDataError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DogfoodPanel {
    Overview,
    Changes,
    Verification,
}

impl DogfoodPanel {
    fn value(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Changes => "changes",
            Self::Verification => "verification",
        }
    }

    fn from_value(value: &str) -> Self {
        match value {
            "overview" => Self::Overview,
            "changes" => Self::Changes,
            "verification" => Self::Verification,
            other => panic!("unknown dogfood panel value: {other}"),
        }
    }
}

#[derive(Clone)]
struct DogfoodEvents {
    data: Rc<RefCell<Result<LocalDogfoodData, String>>>,
    selected_panel: Rc<Cell<DogfoodPanel>>,
    claim_gate_acknowledged: Rc<Cell<bool>>,
    refresh_count: Rc<Cell<u32>>,
    notifier: ViewNotifier,
}

pub struct DogfoodControlsView {
    data: Rc<RefCell<Result<LocalDogfoodData, String>>>,
    selected_panel: Rc<Cell<DogfoodPanel>>,
    claim_gate_acknowledged: Rc<Cell<bool>>,
    refresh_count: Rc<Cell<u32>>,
    finite: bool,
}

impl DogfoodControlsView {
    pub fn new(finite: bool) -> Self {
        Self::with_data_result(
            load_local_dogfood_data().map_err(|err| err.to_string()),
            finite,
        )
    }

    pub fn with_data(data: LocalDogfoodData, finite: bool) -> Self {
        Self::with_data_result(Ok(data), finite)
    }

    pub fn selected_panel(&self) -> DogfoodPanel {
        self.selected_panel.get()
    }

    pub fn claim_gate_acknowledged(&self) -> bool {
        self.claim_gate_acknowledged.get()
    }

    fn with_data_result(data: Result<LocalDogfoodData, String>, finite: bool) -> Self {
        Self {
            data: Rc::new(RefCell::new(data)),
            selected_panel: Rc::new(Cell::new(DogfoodPanel::Overview)),
            claim_gate_acknowledged: Rc::new(Cell::new(false)),
            refresh_count: Rc::new(Cell::new(0)),
            finite,
        }
    }
}

impl View for DogfoodControlsView {
    type Element = ui::Container;

    fn render(&mut self, cx: &mut ViewContext<Self>) -> Self::Element {
        if self.finite {
            cx.app_mut().quit();
        }

        let events = DogfoodEvents {
            data: Rc::clone(&self.data),
            selected_panel: Rc::clone(&self.selected_panel),
            claim_gate_acknowledged: Rc::clone(&self.claim_gate_acknowledged),
            refresh_count: Rc::clone(&self.refresh_count),
            notifier: cx.notifier(),
        };

        match self.data.borrow().clone() {
            Ok(data) => controls_panel_with_events(
                &data,
                self.selected_panel.get(),
                self.claim_gate_acknowledged.get(),
                self.refresh_count.get(),
                Some(events),
            ),
            Err(message) => error_panel(message, events),
        }
    }
}

pub fn controls_panel() -> ui::Container {
    match load_local_dogfood_data() {
        Ok(data) => controls_panel_from_data(&data, DogfoodPanel::Overview, false, 0),
        Err(err) => panic!("failed to load local dogfood data: {err}"),
    }
}

pub fn controls_panel_from_data(
    data: &LocalDogfoodData,
    selected_panel: DogfoodPanel,
    claim_gate_acknowledged: bool,
    refresh_count: u32,
) -> ui::Container {
    controls_panel_with_events(
        data,
        selected_panel,
        claim_gate_acknowledged,
        refresh_count,
        None,
    )
}

fn controls_panel_with_events(
    data: &LocalDogfoodData,
    selected_panel: DogfoodPanel,
    claim_gate_acknowledged: bool,
    refresh_count: u32,
    events: Option<DogfoodEvents>,
) -> ui::Container {
    ui::container()
        .w(760.0)
        .h(520.0)
        .padding(28.0)
        .background(Color::hex(0xf8fafc))
        .child(
            ui::column()
                .spacing(18.0)
                .child(controls_header(data))
                .child(metric_row(data))
                .child(action_row(refresh_count, events.as_ref()))
                .child(setting_row(claim_gate_acknowledged, events.as_ref()))
                .child(
                    ui::progress_bar(verification_progress(data, claim_gate_acknowledged))
                        .width(420.0)
                        .size(ui::ControlSize::Large),
                )
                .child(panel_selector(selected_panel, events.as_ref()))
                .child(panel_body(data, selected_panel, claim_gate_acknowledged)),
        )
}

pub fn load_local_dogfood_data() -> Result<LocalDogfoodData, DogfoodDataError> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml_path = root.join("Cargo.toml");
    let cargo_toml =
        std::fs::read_to_string(&cargo_toml_path).map_err(|err| DogfoodDataError::Io {
            path: cargo_toml_path.clone(),
            message: err.to_string(),
        })?;

    let package_name = read_package_field(&cargo_toml, "name")?;
    let package_version = read_package_field(&cargo_toml, "version")?;
    let advanced_ui_files = count_rust_files(&root.join("src/advanced_ui"))?;
    let integration_tests = count_rust_files(&root.join("tests"))?;
    let examples = count_rust_files(&root.join("examples"))?;
    let git_changes = git_status_lines(root)?;
    let verification_checks = vec![
        file_check(
            root,
            "src/advanced_ui/mod.rs",
            "advanced UI module is wired",
        ),
        file_check(
            root,
            "src/testing/mod.rs",
            "headless testing module is wired",
        ),
        file_check(root, "tests/headless.rs", "headless tests exist"),
        file_check(root, "tests/example_smoke.rs", "example smoke tests exist"),
    ];

    Ok(LocalDogfoodData {
        package_name,
        package_version,
        advanced_ui_files,
        integration_tests,
        examples,
        git_changes,
        verification_checks,
    })
}

fn controls_header(data: &LocalDogfoodData) -> impl Element {
    ui::column()
        .spacing(4.0)
        .child(
            ui::text(format!("{} local dogfood", data.package_name))
                .size(24.0)
                .bold()
                .color(Color::hex(0x111827)),
        )
        .child(
            ui::text(format!(
                "Package {} is rendered from this checkout, not seeded sample data",
                data.package_version
            ))
            .size(13.0)
            .color(Color::hex(0x6b7280)),
        )
}

fn metric_row(data: &LocalDogfoodData) -> impl Element {
    ui::row()
        .spacing(12.0)
        .child(metric_card(
            "Advanced UI files",
            data.advanced_ui_files.to_string(),
            "src/advanced_ui",
            0x2563eb,
        ))
        .child(metric_card(
            "Integration tests",
            data.integration_tests.to_string(),
            "tests/",
            0x059669,
        ))
        .child(metric_card(
            "Examples",
            data.examples.to_string(),
            "examples/",
            0x7c3aed,
        ))
}

fn action_row(refresh_count: u32, events: Option<&DogfoodEvents>) -> impl Element {
    ui::row()
        .spacing(10.0)
        .child(refresh_button(refresh_count, events))
        .child(ui::button("Open issue #28").outline().read_only(true))
        .child(ui::tooltip(
            ui::button("Local proof").ghost().read_only(true),
            "Uses Cargo.toml, file counts, and git status from this checkout",
        ))
}

fn refresh_button(refresh_count: u32, events: Option<&DogfoodEvents>) -> ui::Button {
    let label = if refresh_count == 0 {
        String::from("Refresh local scan")
    } else {
        format!("Refresh local scan ({refresh_count})")
    };
    let button = ui::button(label)
        .id(DOGFOOD_REFRESH_BUTTON_ID)
        .primary()
        .size(ui::ControlSize::Medium);

    match events {
        Some(events) => {
            let events = events.clone();
            button.on_click(move || {
                *events.data.borrow_mut() =
                    load_local_dogfood_data().map_err(|err| err.to_string());
                events
                    .refresh_count
                    .set(events.refresh_count.get().saturating_add(1));
                events.notifier.notify();
            })
        }
        None => button.read_only(true),
    }
}

fn setting_row(claim_gate_acknowledged: bool, events: Option<&DogfoodEvents>) -> impl Element {
    ui::row()
        .spacing(16.0)
        .cross_axis_alignment(ui::CrossAxisAlignment::Center)
        .child(claim_gate_checkbox(claim_gate_acknowledged, events))
        .child(
            ui::checkbox("No seeded placeholders")
                .checked(true)
                .read_only(true),
        )
}

fn claim_gate_checkbox(
    claim_gate_acknowledged: bool,
    events: Option<&DogfoodEvents>,
) -> ui::Checkbox {
    let checkbox = ui::checkbox("Block production claims until checks pass")
        .id(DOGFOOD_CLAIM_GATE_ID)
        .checked(claim_gate_acknowledged);

    match events {
        Some(events) => {
            let events = events.clone();
            checkbox.on_change(move |checked| {
                events.claim_gate_acknowledged.set(checked);
                events.notifier.notify();
            })
        }
        None => checkbox.read_only(true),
    }
}

fn panel_selector(
    selected_panel: DogfoodPanel,
    events: Option<&DogfoodEvents>,
) -> ui::SegmentedControl {
    let control = ui::segmented_control(
        [
            ("overview", "Overview"),
            ("changes", "Changes"),
            ("verification", "Verification"),
        ],
        selected_panel.value(),
    )
    .id(DOGFOOD_PANEL_CONTROL_ID)
    .accessibility_label("Dogfood data panel")
    .size(ui::ControlSize::Medium);

    match events {
        Some(events) => {
            let events = events.clone();
            control.on_change(move |value| {
                events.selected_panel.set(DogfoodPanel::from_value(value));
                events.notifier.notify();
            })
        }
        None => control.read_only(true),
    }
}

fn panel_body(
    data: &LocalDogfoodData,
    selected_panel: DogfoodPanel,
    claim_gate_acknowledged: bool,
) -> ui::Container {
    match selected_panel {
        DogfoodPanel::Overview => overview_panel(data),
        DogfoodPanel::Changes => changes_panel(data),
        DogfoodPanel::Verification => verification_panel(data, claim_gate_acknowledged),
    }
}

fn overview_panel(data: &LocalDogfoodData) -> ui::Container {
    ui::container()
        .w(620.0)
        .h(132.0)
        .padding(16.0)
        .background(Color::WHITE)
        .radius(6.0)
        .child(
            ui::column()
                .spacing(10.0)
                .child(
                    ui::text("Primary screen")
                        .size(18.0)
                        .bold()
                        .color(Color::hex(0x111827)),
                )
                .child(
                    ui::text(format!(
                        "Rendering {} advanced UI source files with {} integration tests in this checkout.",
                        data.advanced_ui_files, data.integration_tests
                    ))
                    .size(14.0)
                    .color(Color::hex(0x374151)),
                )
                .child(
                    ui::text("The app reads repository files and git state at startup.")
                        .size(14.0)
                        .color(Color::hex(0x374151)),
                ),
        )
}

fn changes_panel(data: &LocalDogfoodData) -> ui::Container {
    let rows = data
        .git_changes
        .iter()
        .map(|change| log_row(change))
        .collect::<Vec<_>>();

    ui::container()
        .w(620.0)
        .h(132.0)
        .padding(14.0)
        .background(Color::WHITE)
        .radius(6.0)
        .child(
            ui::scrollable(ui::column().spacing(8.0).children(rows))
                .id(DOGFOOD_ACTIVITY_SCROLL_ID)
                .accessibility_label("Repository git status")
                .w(580.0)
                .h(104.0)
                .background(Color::hex(0xf9fafb)),
        )
}

fn verification_panel(data: &LocalDogfoodData, claim_gate_acknowledged: bool) -> ui::Container {
    let mut rows = data
        .verification_checks
        .iter()
        .map(|check| check_row(&check.label, check.passed))
        .collect::<Vec<_>>();
    rows.push(check_row(
        "production-ready claims remain blocked",
        !claim_gate_acknowledged,
    ));

    ui::container()
        .w(620.0)
        .h(132.0)
        .padding(14.0)
        .background(Color::WHITE)
        .radius(6.0)
        .child(ui::column().spacing(6.0).children(rows))
}

fn metric_card(label: &str, value: String, caption: &str, accent: u32) -> ui::Container {
    ui::container()
        .w(190.0)
        .padding(14.0)
        .background(Color::WHITE)
        .radius(6.0)
        .child(
            ui::column()
                .spacing(6.0)
                .child(
                    ui::text(label)
                        .size(12.0)
                        .medium()
                        .color(Color::hex(0x6b7280)),
                )
                .child(
                    ui::text(value)
                        .size(24.0)
                        .bold()
                        .color(Color::hex(0x111827)),
                )
                .child(
                    ui::text(caption)
                        .size(12.0)
                        .semibold()
                        .color(Color::hex(accent)),
                ),
        )
}

fn log_row(label: &str) -> ui::Container {
    ui::container()
        .w(540.0)
        .h(28.0)
        .padding(6.0)
        .background(Color::hex(0xf3f4f6))
        .radius(4.0)
        .child(ui::text(label).size(12.0).color(Color::hex(0x374151)))
}

fn check_row(label: &str, passed: bool) -> ui::Container {
    let state = if passed { "pass" } else { "blocked" };
    let color = if passed { 0x059669 } else { 0xb45309 };

    ui::container().w(560.0).h(24.0).child(
        ui::row()
            .main_axis_alignment(ui::MainAxisAlignment::SpaceBetween)
            .cross_axis_alignment(ui::CrossAxisAlignment::Center)
            .child(ui::text(label).size(13.0).color(Color::hex(0x374151)))
            .child(
                ui::text(state)
                    .size(12.0)
                    .semibold()
                    .color(Color::hex(color)),
            ),
    )
}

fn error_panel(message: String, events: DogfoodEvents) -> ui::Container {
    ui::container()
        .w(760.0)
        .h(520.0)
        .padding(28.0)
        .background(Color::hex(0xfffbeb))
        .child(
            ui::column()
                .spacing(16.0)
                .child(
                    ui::text("Local dogfood data failed to load")
                        .size(24.0)
                        .bold()
                        .color(Color::hex(0x92400e)),
                )
                .child(ui::text(message).size(14.0).color(Color::hex(0x78350f)))
                .child(refresh_button(events.refresh_count.get(), Some(&events))),
        )
}

fn verification_progress(data: &LocalDogfoodData, claim_gate_acknowledged: bool) -> f32 {
    let passed = data
        .verification_checks
        .iter()
        .filter(|check| check.passed)
        .count()
        + usize::from(!claim_gate_acknowledged);
    let total = data.verification_checks.len() + 1;
    if total == 0 {
        0.0
    } else {
        passed as f32 / total as f32
    }
}

fn read_package_field(cargo_toml: &str, field: &'static str) -> Result<String, DogfoodDataError> {
    let mut in_package = false;
    for line in cargo_toml.lines() {
        let trimmed = line.trim();
        if trimmed == "[package]" {
            in_package = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_package = false;
        }
        if in_package && trimmed.starts_with(field) {
            return parse_quoted_field(trimmed, field);
        }
    }
    Err(DogfoodDataError::MissingPackageField { field })
}

fn parse_quoted_field(line: &str, field: &'static str) -> Result<String, DogfoodDataError> {
    let prefix = format!("{field} = ");
    let raw = match line.strip_prefix(&prefix) {
        Some(value) => value.trim(),
        None => return Err(DogfoodDataError::MissingPackageField { field }),
    };
    if raw.len() < 2 || !raw.starts_with('"') || !raw.ends_with('"') {
        return Err(DogfoodDataError::InvalidPackageField { field });
    }
    Ok(raw[1..raw.len() - 1].to_string())
}

fn count_rust_files(path: &Path) -> Result<usize, DogfoodDataError> {
    let mut count = 0;
    let mut pending = vec![path.to_path_buf()];
    while let Some(current) = pending.pop() {
        let entries = std::fs::read_dir(&current).map_err(|err| DogfoodDataError::Io {
            path: current.clone(),
            message: err.to_string(),
        })?;
        for entry in entries {
            let entry = entry.map_err(|err| DogfoodDataError::Io {
                path: current.clone(),
                message: err.to_string(),
            })?;
            let entry_path = entry.path();
            let file_type = entry.file_type().map_err(|err| DogfoodDataError::Io {
                path: entry_path.clone(),
                message: err.to_string(),
            })?;
            if file_type.is_dir() {
                pending.push(entry_path);
            } else if entry_path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
                count += 1;
            }
        }
    }
    Ok(count)
}

fn git_status_lines(root: &Path) -> Result<Vec<String>, DogfoodDataError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["status", "--short"])
        .output()
        .map_err(|err| DogfoodDataError::Command {
            command: String::from("git status --short"),
            message: err.to_string(),
        })?;

    if !output.status.success() {
        let stderr = utf8_text("git stderr", output.stderr)?;
        return Err(DogfoodDataError::Command {
            command: String::from("git status --short"),
            message: stderr,
        });
    }

    let stdout = utf8_text("git stdout", output.stdout)?;
    let lines = stdout
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(String::from)
        .collect::<Vec<_>>();

    if lines.is_empty() {
        Ok(vec![String::from("working tree clean")])
    } else {
        Ok(lines)
    }
}

fn utf8_text(source: &'static str, bytes: Vec<u8>) -> Result<String, DogfoodDataError> {
    String::from_utf8(bytes).map_err(|err| DogfoodDataError::Utf8 {
        source,
        message: err.to_string(),
    })
}

fn file_check(root: &Path, relative_path: &str, label: &str) -> LocalVerificationCheck {
    LocalVerificationCheck {
        label: String::from(label),
        passed: root.join(relative_path).is_file(),
    }
}
