//! Dashboard example - demonstrates a complete UI application
//!
//! This example shows a dashboard layout with:
//! - Sidebar navigation
//! - Cards with statistics
//! - Buttons with different variants
//! - Input fields
//! - ScrollView for long content

use rui::prelude::*;

fn main() {
    App::new().run(|_cx| {
        // Main container - horizontal layout
        div()
            .w(1200.0)
            .h(800.0)
            .bg(Color::hex(0xf3f4f6))
            .flex_row()
            .child(sidebar())
            .child(main_content())
    });
}

/// Sidebar navigation
fn sidebar() -> Div {
    div()
        .w(240.0)
        .h(800.0)
        .bg(Color::hex(0x1f2937))
        .flex_col()
        .p(16.0)
        .gap(8.0)
        // Logo
        .child(
            div()
                .py(16.0)
                .child(
                    text("RUI Dashboard")
                        .size(20.0)
                        .bold()
                        .color(Color::WHITE),
                ),
        )
        // Divider
        .child(
            div()
                .h(1.0)
                .w_full()
                .bg(Color::hex(0x374151))
                .my(8.0),
        )
        // Nav items
        .child(nav_item("Home", true))
        .child(nav_item("Analytics", false))
        .child(nav_item("Projects", false))
        .child(nav_item("Team", false))
        .child(nav_item("Settings", false))
        // Spacer
        .child(div().flex_grow(1.0))
        // User section
        .child(
            div()
                .flex_row()
                .items_center()
                .gap(12.0)
                .p(8.0)
                .rounded(8.0)
                .bg(Color::hex(0x374151))
                .child(
                    div()
                        .w(36.0)
                        .h(36.0)
                        .rounded_full()
                        .bg(Color::hex(0x6366f1))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(text("JD").size(14.0).bold().color(Color::WHITE)),
                )
                .child(
                    div()
                        .flex_col()
                        .child(text("John Doe").size(14.0).color(Color::WHITE))
                        .child(text("Admin").size(12.0).color(Color::hex(0x9ca3af))),
                ),
        )
}

/// Navigation item
fn nav_item(label: &str, active: bool) -> Div {
    let bg = if active {
        Color::hex(0x374151)
    } else {
        Color::TRANSPARENT
    };
    let text_color = if active {
        Color::WHITE
    } else {
        Color::hex(0x9ca3af)
    };

    div()
        .flex_row()
        .items_center()
        .gap(12.0)
        .px(12.0)
        .py(10.0)
        .rounded(8.0)
        .bg(bg)
        .child(
            div()
                .w(8.0)
                .h(8.0)
                .rounded_full()
                .bg(if active {
                    Color::hex(0x6366f1)
                } else {
                    Color::hex(0x4b5563)
                }),
        )
        .child(text(label).size(14.0).color(text_color))
}

/// Main content area
fn main_content() -> Div {
    div()
        .flex_grow(1.0)
        .h(800.0)
        .flex_col()
        .p(24.0)
        .gap(24.0)
        // Header
        .child(header())
        // Stats cards
        .child(stats_section())
        // Action section
        .child(action_section())
        // Content area
        .child(content_section())
}

/// Header with title and search
fn header() -> Div {
    div()
        .flex_row()
        .justify_between()
        .items_center()
        .child(
            div()
                .flex_col()
                .gap(4.0)
                .child(text("Dashboard").size(28.0).bold().color(Color::hex(0x111827)))
                .child(text("Welcome back, John!").size(14.0).color(Color::hex(0x6b7280))),
        )
        .child(
            div()
                .flex_row()
                .gap(12.0)
                .child(
                    input()
                        .placeholder("Search...")
                        .w(280.0)
                        .search(),
                )
                .child(button("+ New Project").primary()),
        )
}

/// Statistics cards section
fn stats_section() -> Div {
    div()
        .flex_row()
        .gap(16.0)
        .child(stat_card("Total Revenue", "$45,231", "+20.1%", true))
        .child(stat_card("Active Users", "2,350", "+15.3%", true))
        .child(stat_card("Pending Tasks", "12", "-4.5%", false))
        .child(stat_card("Completed", "89%", "+2.4%", true))
}

/// Single stat card
fn stat_card(title: &str, value: &str, change: &str, positive: bool) -> Div {
    let change_color = if positive {
        Color::hex(0x22c55e)
    } else {
        Color::hex(0xef4444)
    };

    div()
        .flex_grow(1.0)
        .bg(Color::WHITE)
        .rounded(12.0)
        .p(20.0)
        .shadow_sm()
        .flex_col()
        .gap(8.0)
        .child(text(title).size(14.0).color(Color::hex(0x6b7280)))
        .child(
            div()
                .flex_row()
                .items_end()
                .gap(8.0)
                .child(text(value).size(28.0).bold().color(Color::hex(0x111827)))
                .child(text(change).size(14.0).semibold().color(change_color)),
        )
}

/// Action buttons section
fn action_section() -> Div {
    div()
        .flex_row()
        .gap(12.0)
        .child(button("Primary").primary())
        .child(button("Secondary").secondary())
        .child(button("Outline").outline())
        .child(button("Ghost").ghost())
        .child(button("Danger").danger())
        .child(button("Success").success())
}

/// Content area with form and info
fn content_section() -> Div {
    div()
        .flex_row()
        .gap(24.0)
        .flex_grow(1.0)
        // Form section
        .child(
            div()
                .w(400.0)
                .bg(Color::WHITE)
                .rounded(12.0)
                .p(24.0)
                .shadow_sm()
                .flex_col()
                .gap(20.0)
                .child(text("Create Project").size(18.0).bold().color(Color::hex(0x111827)))
                .child(form_field("Project Name", "Enter project name"))
                .child(form_field("Description", "Enter description"))
                .child(form_field("Team Lead", "Select team lead"))
                .child(
                    div()
                        .flex_row()
                        .gap(12.0)
                        .justify_end()
                        .child(button("Cancel").outline())
                        .child(button("Create").primary()),
                ),
        )
        // Info cards
        .child(
            div()
                .flex_grow(1.0)
                .flex_col()
                .gap(16.0)
                .child(info_card(
                    "Recent Activity",
                    "5 new commits pushed to main branch",
                ))
                .child(info_card("Team Updates", "Sarah joined the design team"))
                .child(info_card("Reminders", "Sprint review tomorrow at 10am")),
        )
}

/// Form field with label and input
fn form_field(label: &str, placeholder: &str) -> Div {
    div()
        .flex_col()
        .gap(6.0)
        .child(text(label).size(14.0).semibold().color(Color::hex(0x374151)))
        .child(input().placeholder(placeholder))
}

/// Info card
fn info_card(title: &str, content: &str) -> Div {
    div()
        .bg(Color::WHITE)
        .rounded(12.0)
        .p(16.0)
        .shadow_sm()
        .flex_col()
        .gap(8.0)
        .child(text(title).size(14.0).semibold().color(Color::hex(0x111827)))
        .child(text(content).size(14.0).color(Color::hex(0x6b7280)))
}
