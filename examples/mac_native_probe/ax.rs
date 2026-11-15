//! The NSAccessibility tree as an assistive app reads it from the window,
//! checked role by role, plus focus order and AX actions.

use crate::native::{self, Ax};
use crate::probe::{Probe, Step, step};
use crate::view;
use objc2::rc::Retained;
use objc2_app_kit::NSWindow;
use objc2_foundation::NSRect;

/// One element as read through the legacy attribute API.
pub struct Node {
    pub object: Ax,
    pub depth: usize,
    pub role: String,
    pub subrole: String,
    pub label: String,
    pub title: String,
    pub value: String,
    pub frame: NSRect,
}

fn read(object: Ax, depth: usize) -> Node {
    let text = |name| native::ax_string(&object, name).unwrap_or_default();
    Node {
        role: text("AXRole"),
        subrole: text("AXSubrole"),
        label: native::ax_label(&object).unwrap_or_default(),
        title: text("AXTitle"),
        value: text("AXValue"),
        frame: native::ax_frame(&object),
        depth,
        object,
    }
}

pub fn walk(window: &NSWindow) -> Vec<Node> {
    fn visit(object: Ax, depth: usize, nodes: &mut Vec<Node>) {
        let children = native::ax_children(&object);
        nodes.push(read(object, depth));
        for child in children {
            visit(child, depth + 1, nodes);
        }
    }
    let mut nodes = Vec::new();
    for child in native::ax_children(window) {
        visit(child, 0, &mut nodes);
    }
    nodes
}

pub fn find_by_label(window: &NSWindow, label: &str) -> Option<Ax> {
    walk(window)
        .into_iter()
        .find(|node| node.label == label)
        .map(|node| node.object)
}

/// Whether `element` is `ancestor` or below it, following AXParent.
fn is_within(element: &Ax, ancestor: &Ax) -> bool {
    let mut current = Some(element.clone());
    for _ in 0..16 {
        let Some(node) = current else { return false };
        if Retained::as_ptr(&node) == Retained::as_ptr(ancestor) {
            return true;
        }
        current = native::ax_attr(&node, "AXParent");
    }
    false
}

fn find<'a>(nodes: &'a [Node], label: &str) -> Option<&'a Node> {
    nodes.iter().find(|node| node.label == label)
}

fn name(node: &Node) -> &str {
    if node.label.is_empty() {
        &node.title
    } else {
        &node.label
    }
}

fn print_tree(nodes: &[Node]) {
    println!("---- AX tree from NSWindow ----");
    for node in nodes {
        let object = &node.object;
        let mut extra = String::new();
        for attribute in [
            "AXHelp",
            "AXEnabled",
            "AXFocused",
            "AXMinValue",
            "AXMaxValue",
        ] {
            if let Some(value) = native::ax_string(object, attribute) {
                extra.push_str(&format!(" {attribute}={value}"));
            }
        }
        if node.role == "AXTextField" || node.role == "AXTextArea" {
            for attribute in [
                "AXSelectedTextRange",
                "AXNumberOfCharacters",
                "AXInsertionPointLineNumber",
            ] {
                let value = native::ax_string(object, attribute).unwrap_or("-".into());
                extra.push_str(&format!(" {attribute}={value}"));
            }
        }
        let actions = native::ax_actions(object);
        println!(
            "{:indent$}{} sub={} label={:?} title={:?} value={:?} frame={}{extra} actions={actions:?}",
            "",
            node.role,
            if node.subrole.is_empty() {
                "-"
            } else {
                &node.subrole
            },
            node.label,
            node.title,
            node.value.replace('\n', "\\n"),
            native::fmt_rect(node.frame),
            indent = node.depth * 2
        );
    }
}

/// The window's own buttons, which AppKit draws and names by role.
const WINDOW_CHROME: [&str; 4] = [
    "AXCloseButton",
    "AXMinimizeButton",
    "AXFullScreenButton",
    "AXZoomButton",
];

const NAMED_ROLES: [&str; 12] = [
    "AXButton",
    "AXCheckBox",
    "AXRadioButton",
    "AXRadioGroup",
    "AXPopUpButton",
    "AXSlider",
    "AXTextField",
    "AXTextArea",
    "AXTabGroup",
    "AXSplitter",
    "AXCell",
    "AXMenuItem",
];

fn expect_node(
    probe: &mut Probe,
    nodes: &[Node],
    label: &str,
    role: &str,
    subrole: &str,
    value: Option<&str>,
) {
    let name = format!(
        "AX {label}: {role}{}",
        if subrole.is_empty() {
            String::new()
        } else {
            format!("/{subrole}")
        }
    );
    let Some(node) = find(nodes, label) else {
        probe.record(name, false, "not in the tree");
        return;
    };
    let pass = node.role == role
        && (subrole.is_empty() || node.subrole == subrole)
        && value.is_none_or(|value| node.value == value);
    probe.record(
        name,
        pass,
        format!(
            "role={} subrole={} value={:?}",
            node.role, node.subrole, node.value
        ),
    );
}

fn static_checks(probe: &mut Probe) {
    let nodes = walk(&probe.window);
    print_tree(&nodes);

    let unnamed: Vec<String> = nodes
        .iter()
        .filter(|node| {
            NAMED_ROLES.contains(&node.role.as_str())
                && !WINDOW_CHROME.contains(&node.subrole.as_str())
                && name(node).trim().is_empty()
        })
        .map(|node| format!("{} at {}", node.role, native::fmt_rect(node.frame)))
        .collect();
    probe.record(
        "every control has a label",
        unnamed.is_empty(),
        format!("{unnamed:?}"),
    );

    let window = probe.window.frame();
    let mut misplaced = Vec::new();
    for node in nodes
        .iter()
        .filter(|node| NAMED_ROLES.contains(&node.role.as_str()))
    {
        let on_screen = node.frame.size.width > 0.0 && native::contains(window, node.frame, 1.0);
        let hit = native::hit_test(&probe.window, native::center(node.frame));
        let hit_name = hit.as_ref().map(|hit| read(hit.clone(), 0));
        // The hit is the element or, over a group, the child under the point.
        let hits_self = hit.as_ref().is_some_and(|hit| is_within(hit, &node.object));
        if !on_screen || !hits_self {
            misplaced.push(format!(
                "{} {:?} frame={} hit={:?}",
                node.role,
                name(node),
                native::fmt_rect(node.frame),
                hit_name.map(|hit| format!("{} {:?}", hit.role, name(&hit)))
            ));
        }
    }
    probe.record(
        "frames are inside the window and hit-test to themselves",
        misplaced.is_empty(),
        format!(
            "window={} misplaced={misplaced:#?}",
            native::fmt_rect(window)
        ),
    );

    expect_node(
        probe,
        &nodes,
        "Notifications",
        "AXCheckBox",
        "AXSwitch",
        Some("1"),
    );
    expect_node(probe, &nodes, "Remember me", "AXCheckBox", "", Some("0"));
    expect_node(probe, &nodes, "Density", "AXRadioGroup", "", None);
    expect_node(probe, &nodes, "Comfortable", "AXRadioButton", "", Some("1"));
    expect_node(probe, &nodes, "Compact", "AXRadioButton", "", Some("0"));
    expect_node(probe, &nodes, "Volume", "AXSlider", "", Some("40"));
    expect_node(probe, &nodes, "Fruit", "AXPopUpButton", "", Some("Banana"));
    expect_node(probe, &nodes, "Sections", "AXTabGroup", "", None);
    expect_node(
        probe,
        &nodes,
        "General",
        "AXRadioButton",
        "AXTabButton",
        Some("1"),
    );
    expect_node(probe, &nodes, "Save preferences", "AXButton", "", None);
    expect_node(
        probe,
        &nodes,
        view::EMULATED,
        "AXTextField",
        "",
        Some("你好"),
    );
    expect_node(probe, &nodes, view::NOTES, "AXTextArea", "", None);

    text_field_checks(probe, &nodes);
    let hidden: Vec<String> = nodes
        .iter()
        .filter(|node| node.role == "AXMenuItem" || node.role == "AXGrid" || node.role == "AXMenu")
        .map(|node| format!("{} {:?}", node.role, name(node)))
        .collect();
    probe.record(
        "closed select and date picker expose no menu or grid",
        hidden.is_empty(),
        format!("{hidden:?}"),
    );
    let panels: Vec<String> = nodes
        .iter()
        .filter(|node| node.role == "AXGroup" && ["General", "Advanced"].contains(&name(node)))
        .map(|node| format!("{} {:?}", node.role, name(node)))
        .collect();
    probe.record(
        "only the selected tab panel is exposed",
        panels.len() == 1,
        format!("{panels:?}"),
    );
}

fn text_field_checks(probe: &mut Probe, nodes: &[Node]) {
    // Each field was typed into last at its end: the caret is a collapsed
    // selection after the last UTF-16 unit of the value.
    for label in [view::EMULATED, view::NOTES] {
        let Some(node) = find(nodes, label) else {
            probe.record(
                format!("AX {label}: text semantics"),
                false,
                "not in the tree",
            );
            continue;
        };
        let length = node.value.encode_utf16().count();
        let selected = native::ax_selected_range(&node.object);
        let count = native::ax_string(&node.object, "AXNumberOfCharacters").unwrap_or_default();
        let line =
            native::ax_string(&node.object, "AXInsertionPointLineNumber").unwrap_or_default();
        let pass =
            (selected.location, selected.length) == (length, 0) && count == length.to_string();
        probe.record(
            format!("AX {label}: value, selected range and insertion point"),
            pass,
            format!(
                "AXValue={:?} AXSelectedTextRange={selected:?} AXNumberOfCharacters={count} \
                 AXInsertionPointLineNumber={line}",
                node.value
            ),
        );
    }
}

/// Press Tab repeatedly and read the app's focused element each time.
fn focus_order() -> Vec<Step> {
    let mut steps = vec![crate::ime::focus(view::COMMIT), record_focus()];
    for _ in 0..TAB_STOPS {
        steps.push(step(|probe| {
            native::post_key(&probe.window, native::TAB);
            250
        }));
        steps.push(record_focus());
    }
    steps.push(step(|probe| {
        let order = FOCUS.with(|focus| focus.take());
        let nodes = walk(&probe.window);
        let positions: Vec<Option<usize>> = order
            .iter()
            .map(|(role, label)| {
                nodes
                    .iter()
                    .position(|node| &node.role == role && name(node) == label)
            })
            .collect();
        // Tab moves forward through the tree, wrapping at most once.
        let backwards = positions
            .windows(2)
            .filter(|pair| !matches!(pair, [Some(a), Some(b)] if b > a))
            .count();
        let order: Vec<String> = order
            .iter()
            .map(|(role, label)| format!("{role} {label:?}"))
            .collect();
        probe.record(
            "Tab order follows the tree and the app reports the focused element",
            backwards <= 1 && positions.iter().all(Option::is_some),
            format!("{order:?}"),
        );
        0
    }));
    steps
}

const TAB_STOPS: usize = 22;

thread_local! {
    static FOCUS: std::cell::RefCell<Vec<(String, String)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn record_focus() -> Step {
    step(|_| {
        FOCUS.with(|focus| focus.borrow_mut().push(focused()));
        0
    })
}

/// Role and name of `NSApp accessibilityFocusedUIElement`.
fn focused() -> (String, String) {
    native::app_focused()
        .map(|element| {
            let node = read(element, 0);
            (node.role.clone(), name(&node).to_string())
        })
        .unwrap_or_else(|| ("none".to_string(), String::new()))
}

/// Perform an AX action on `label`, then check the tree after the frame.
fn act(
    label: &'static str,
    action: fn(&objc2::runtime::AnyObject) -> bool,
    verdict: fn(&[Node]) -> (bool, String),
) -> Step {
    step(move |probe| {
        let accepted = find_by_label(&probe.window, label).is_some_and(|element| action(&element));
        probe.push_front(vec![step(move |probe| {
            let nodes = walk(&probe.window);
            let (pass, detail) = verdict(&nodes);
            probe.record(
                format!("AX action on {label}"),
                accepted && pass,
                format!("accepted={accepted} {detail}"),
            );
            0
        })]);
        500
    })
}

fn value_of(nodes: &[Node], label: &str) -> String {
    find(nodes, label)
        .map(|node| node.value.clone())
        .unwrap_or_default()
}

pub fn steps() -> Vec<Step> {
    let mut steps = vec![step(|probe| {
        static_checks(probe);
        0
    })];
    steps.extend(focus_order());
    steps.push(act("Save preferences", native::press, |nodes| {
        let toast: Vec<String> = nodes
            .iter()
            .filter(|node| {
                name(node).contains("Preferences saved") || node.value.contains("Preferences saved")
            })
            .map(|node| format!("{} sub={} {:?}", node.role, node.subrole, name(node)))
            .collect();
        (
            view::saves() == 1 && !toast.is_empty(),
            format!("saves={} toast={toast:?}", view::saves()),
        )
    }));
    steps.push(act("Notifications", native::press, |nodes| {
        let value = value_of(nodes, "Notifications");
        (value == "0", format!("value={value}"))
    }));
    steps.push(act("Remember me", native::press, |nodes| {
        let value = value_of(nodes, "Remember me");
        (value == "1", format!("value={value}"))
    }));
    steps.push(act("Volume", native::increment, |nodes| {
        let value = value_of(nodes, "Volume");
        (value == "45", format!("value={value}"))
    }));
    steps.push(act("Spacious", native::press, |nodes| {
        let (spacious, comfortable) = (value_of(nodes, "Spacious"), value_of(nodes, "Comfortable"));
        (
            spacious == "1" && comfortable == "0",
            format!("Spacious={spacious} Comfortable={comfortable}"),
        )
    }));
    steps.push(act("Advanced", native::press, |nodes| {
        let advanced = value_of(nodes, "Advanced");
        (advanced == "1", format!("Advanced={advanced}"))
    }));
    steps.push(act("Due date", native::press, |nodes| {
        let cells: Vec<&Node> = nodes.iter().filter(|node| node.role == "AXCell").collect();
        let grid = nodes
            .iter()
            .find(|node| node.role == "AXGrid")
            .map(|node| name(node).to_string());
        let unnamed = cells.iter().filter(|cell| name(cell).is_empty()).count();
        let sample: Vec<String> = cells
            .iter()
            .take(3)
            .map(|cell| format!("{:?} value={:?}", name(cell), cell.value))
            .collect();
        (
            grid.is_some() && cells.len() >= 28 && unnamed == 0,
            format!(
                "grid={grid:?} cells={} unnamed={unnamed} sample={sample:?}",
                cells.len()
            ),
        )
    }));
    steps.push(step(|probe| {
        native::post_key(&probe.window, native::ESCAPE);
        400
    }));
    steps.push(step(|probe| {
        print_tree(&walk(&probe.window));
        0
    }));
    steps
}
