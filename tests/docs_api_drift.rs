const API_DOC: &str = include_str!("../docs/API.md");
const README: &str = include_str!("../README.md");

struct EnumSurface {
    name: &'static str,
    source: &'static str,
}

#[test]
fn docs_api_covers_public_builders_and_validation_commands() {
    let builders = [
        "div()",
        "text(",
        "button(",
        "input()",
        "image(",
        "scroll_view()",
        "table()",
        "row()",
        "header_row()",
        "cell(",
        "list()",
        "ordered_list()",
        "unordered_list()",
        "progress()",
        "spinner()",
        "advanced_ui::container()",
        "advanced_ui::row()",
        "advanced_ui::column()",
        "advanced_ui::button(",
        "advanced_ui::checkbox(",
        "advanced_ui::progress_bar(",
        "advanced_ui::scrollable(",
        "advanced_ui::segmented_control(",
        "advanced_ui::toolbar(",
        "advanced_ui::tooltip(",
    ];

    for builder in builders {
        assert!(
            API_DOC.contains(builder),
            "docs/API.md should document public builder `{builder}`"
        );
    }

    for command in [
        "cargo test docs_api_drift",
        "cargo test example_smoke",
        "cargo test dogfood",
        "scripts/native_dogfood_macos.sh",
    ] {
        assert!(
            README.contains(command) || API_DOC.contains(command),
            "README.md or docs/API.md should document validation command `{command}`"
        );
    }
}

#[test]
fn docs_api_enum_variants_match_source() {
    let surfaces = [
        EnumSurface {
            name: "ButtonVariant",
            source: include_str!("../src/elements/button.rs"),
        },
        EnumSurface {
            name: "ButtonSize",
            source: include_str!("../src/elements/button.rs"),
        },
        EnumSurface {
            name: "InputType",
            source: include_str!("../src/elements/input.rs"),
        },
        EnumSurface {
            name: "ImageFit",
            source: include_str!("../src/elements/image.rs"),
        },
        EnumSurface {
            name: "ScrollDirection",
            source: include_str!("../src/elements/scroll_view.rs"),
        },
        EnumSurface {
            name: "ScrollbarVisibility",
            source: include_str!("../src/elements/scroll_view.rs"),
        },
        EnumSurface {
            name: "ListStyle",
            source: include_str!("../src/elements/list.rs"),
        },
        EnumSurface {
            name: "SpinnerType",
            source: include_str!("../src/elements/spinner.rs"),
        },
        EnumSurface {
            name: "TextAlign",
            source: include_str!("../src/elements/text/mod.rs"),
        },
        EnumSurface {
            name: "Easing",
            source: include_str!("../src/core/animation.rs"),
        },
        EnumSurface {
            name: "ControlSize",
            source: include_str!("../src/advanced_ui/tokens.rs"),
        },
        EnumSurface {
            name: "ControlVariant",
            source: include_str!("../src/advanced_ui/tokens.rs"),
        },
        EnumSurface {
            name: "MainAxisAlignment",
            source: include_str!("../src/advanced_ui/mod.rs"),
        },
        EnumSurface {
            name: "CrossAxisAlignment",
            source: include_str!("../src/advanced_ui/mod.rs"),
        },
    ];

    for surface in surfaces {
        let variants = enum_variants(surface.source, surface.name);
        assert!(
            !variants.is_empty(),
            "test fixture should find enum {} variants",
            surface.name
        );

        for variant in variants {
            let token = format!("{}::{}", surface.name, variant);
            assert!(
                API_DOC.contains(&token),
                "docs/API.md should mention `{token}`"
            );
        }
    }
}

fn enum_variants(source: &str, enum_name: &str) -> Vec<String> {
    let Some(enum_start) = source.find(&format!("pub enum {enum_name}")) else {
        return Vec::new();
    };
    let body_start = source[enum_start..]
        .find('{')
        .map(|offset| enum_start + offset + 1)
        .expect("public enum should have a body");
    let body_end = matching_brace_end(source, body_start - 1).expect("public enum body should end");

    source[body_start..body_end]
        .lines()
        .filter_map(variant_name)
        .collect()
}

fn matching_brace_end(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, ch) in source[open..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }
    None
}

fn variant_name(line: &str) -> Option<String> {
    let trimmed = line.split("//").next().unwrap_or("").trim();
    if trimmed.is_empty() || trimmed.starts_with("#[") || trimmed.starts_with("///") {
        return None;
    }

    let name = trimmed
        .trim_end_matches(',')
        .split([' ', '{', '('])
        .next()
        .unwrap_or("");
    if name.chars().next().is_some_and(char::is_uppercase) {
        Some(String::from(name))
    } else {
        None
    }
}
