fn action_for_selector(selector: Sel) -> Option<AccessibilityAction> {
    if selector == sel!(accessibilityPerformPress) {
        Some(AccessibilityAction::Activate)
    } else if selector == sel!(setAccessibilityValue:) {
        Some(AccessibilityAction::SetValue)
    } else if selector == sel!(accessibilityPerformDecrement) {
        Some(AccessibilityAction::ScrollBackward)
    } else if selector == sel!(accessibilityPerformIncrement) {
        Some(AccessibilityAction::ScrollForward)
    } else {
        None
    }
}

fn native_frame_in_parent(bounds: Bounds, parent: Bounds) -> NSRect {
    let x = bounds.x() - parent.x();
    let y = parent.height() - (bounds.y() - parent.y()) - bounds.height();
    NSRect::new(
        NSPoint::new(x as f64, y as f64),
        NSSize::new(bounds.width() as f64, bounds.height() as f64),
    )
}

fn publish_text_semantics(
    element: &RuiAccessibilityElement,
    node: &AccessibilityNode,
) -> Result<(), AccessibilityError> {
    if node.a11y_role() != AccessibilityRole::TextInput {
        element.setAccessibilityNumberOfCharacters(0);
        element.setAccessibilitySelectedText(None);
        element.setAccessibilitySelectedTextRange(NSRange::new(0, 0));
        element.setAccessibilityVisibleCharacterRange(NSRange::new(0, 0));
        return Ok(());
    }

    let value = node.a11y_value().unwrap_or_default();
    let native_length = value.encode_utf16().count();
    element.setAccessibilityNumberOfCharacters(native_length as isize);
    element.setAccessibilityVisibleCharacterRange(NSRange::new(0, native_length));

    let range = node
        .a11y_text_selection()
        .map(|range| (range.start(), range.end()))
        .or_else(|| node.a11y_text_caret().map(|caret| (caret, caret)))
        .unwrap_or((0, 0));
    let native_range = native_text_range(value, range.0, range.1)?;
    element.setAccessibilitySelectedTextRange(native_range);
    let selected = value
        .get(range.0..range.1)
        .ok_or_else(|| invalid_text_range(range.0, range.1, value.len()))?;
    let selected = NSString::from_str(selected);
    element.setAccessibilitySelectedText(Some(&selected));
    Ok(())
}

fn publish_native_value(
    element: &RuiAccessibilityElement,
    node: &AccessibilityNode,
) -> Result<(), AccessibilityError> {
    match node.a11y_role() {
        AccessibilityRole::Checkbox => {
            let value = NSNumber::new_bool(node.a11y_checked().unwrap_or(false));
            element.set_native_value(Some(&value));
        }
        AccessibilityRole::SegmentedOption | AccessibilityRole::Tab => {
            let value = NSNumber::new_bool(node.a11y_selected().unwrap_or(false));
            element.set_native_value(Some(&value));
        }
        AccessibilityRole::ProgressIndicator => {
            let source = node.a11y_value().unwrap_or_default();
            let value = source
                .strip_suffix('%')
                .and_then(|value| value.parse::<f64>().ok())
                .map(|value| value / 100.0)
                .ok_or_else(|| AccessibilityError::BridgeFailure {
                    message: format!("invalid macOS progress accessibility value: {source}"),
                })?;
            let value = NSNumber::new_f64(value);
            element.set_native_value(Some(&value));
        }
        _ => {
            let value = node.a11y_value().map(NSString::from_str);
            element.set_native_value(value.as_deref().map(|value| value as &AnyObject));
        }
    }
    Ok(())
}

fn publish_value_description(element: &RuiAccessibilityElement, node: &AccessibilityNode) {
    let description = if let Some(position) = node.a11y_scroll_position() {
        Some(format!(
            "horizontal {} of {}, vertical {} of {}",
            position.offset_x(),
            position.max_x(),
            position.offset_y(),
            position.max_y()
        ))
    } else if matches!(
        node.a11y_role(),
        AccessibilityRole::ProgressIndicator
            | AccessibilityRole::SegmentedOption
            | AccessibilityRole::Tab
    ) {
        node.a11y_value()
            .or(node.a11y_label())
            .map(str::to_string)
    } else {
        None
    };
    let description = description.as_deref().map(NSString::from_str);
    element.setAccessibilityValueDescription(description.as_deref());
}

fn accessibility_layout_changed(
    previous: &AccessibilityTree,
    current: &AccessibilityTree,
) -> bool {
    previous.roots().len() != current.roots().len()
        || previous
            .roots()
            .iter()
            .zip(current.roots())
            .any(|(previous, current)| accessibility_node_layout_changed(previous, current))
}

fn accessibility_node_layout_changed(
    previous: &AccessibilityNode,
    current: &AccessibilityNode,
) -> bool {
    previous.a11y_id() != current.a11y_id()
        || previous.a11y_role() != current.a11y_role()
        || previous.a11y_bounds() != current.a11y_bounds()
        || previous.a11y_children().len() != current.a11y_children().len()
        || previous
            .a11y_children()
            .iter()
            .zip(current.a11y_children())
            .any(|(previous, current)| accessibility_node_layout_changed(previous, current))
}

fn post_property_notifications(
    previous: &AccessibilityTree,
    current: &AccessibilityTree,
    elements: &HashMap<ElementId, Retained<RuiAccessibilityElement>>,
) {
    for node in current.roots() {
        post_node_property_notifications(previous, node, elements);
    }
}

fn post_node_property_notifications(
    previous: &AccessibilityTree,
    current: &AccessibilityNode,
    elements: &HashMap<ElementId, Retained<RuiAccessibilityElement>>,
) {
    if let (Some(previous), Some(element)) = (
        find_accessibility_node(previous.roots(), current.a11y_id()),
        elements.get(&current.a11y_id()),
    ) {
        let element: &AnyObject = element;
        unsafe {
            if previous.a11y_value() != current.a11y_value()
                || previous.a11y_checked() != current.a11y_checked()
                || previous.a11y_selected() != current.a11y_selected()
            {
                NSAccessibilityPostNotification(element, NSAccessibilityValueChangedNotification);
            }
            if previous.a11y_label() != current.a11y_label() {
                NSAccessibilityPostNotification(element, NSAccessibilityTitleChangedNotification);
            }
            if previous.a11y_text_caret() != current.a11y_text_caret()
                || previous.a11y_text_selection() != current.a11y_text_selection()
                || previous.a11y_text_composition() != current.a11y_text_composition()
            {
                NSAccessibilityPostNotification(
                    element,
                    NSAccessibilitySelectedTextChangedNotification,
                );
            }
        }
    }
    for child in current.a11y_children() {
        post_node_property_notifications(previous, child, elements);
    }
}

fn find_accessibility_node(
    nodes: &[AccessibilityNode],
    id: ElementId,
) -> Option<&AccessibilityNode> {
    nodes.iter().find_map(|node| {
        (node.a11y_id() == id)
            .then_some(node)
            .or_else(|| find_accessibility_node(node.a11y_children(), id))
    })
}

fn native_text_range(
    value: &str,
    start: usize,
    end: usize,
) -> Result<NSRange, AccessibilityError> {
    let prefix = value
        .get(..start)
        .ok_or_else(|| invalid_text_range(start, end, value.len()))?;
    let selected = value
        .get(start..end)
        .ok_or_else(|| invalid_text_range(start, end, value.len()))?;
    Ok(NSRange::new(
        prefix.encode_utf16().count(),
        selected.encode_utf16().count(),
    ))
}

fn invalid_text_range(start: usize, end: usize, value_len: usize) -> AccessibilityError {
    AccessibilityError::BridgeFailure {
        message: format!(
            "invalid accessibility text range {start}..{end} for UTF-8 value length {value_len}"
        ),
    }
}

fn validate_tree(tree: &AccessibilityTree) -> Result<(), AccessibilityError> {
    for node in tree.roots() {
        validate_node(node)?;
    }
    Ok(())
}

fn validate_node(node: &AccessibilityNode) -> Result<(), AccessibilityError> {
    let role = node.a11y_role();
    if role_requires_label(role)
        && node
            .a11y_label()
            .is_none_or(|label| label.trim().is_empty())
    {
        return Err(AccessibilityError::MissingLabel { role });
    }
    if role_requires_value(role)
        && node
            .a11y_value()
            .is_none_or(|value| role != AccessibilityRole::TextInput && value.trim().is_empty())
    {
        return Err(AccessibilityError::MissingValue { role });
    }
    if role == AccessibilityRole::TextInput {
        let value = node.a11y_value().unwrap_or_default();
        if let Some(caret) = node.a11y_text_caret() {
            native_text_range(value, caret, caret)?;
        }
        for range in [node.a11y_text_selection(), node.a11y_text_composition()]
            .into_iter()
            .flatten()
        {
            native_text_range(value, range.start(), range.end())?;
        }
    }
    for child in node.a11y_children() {
        validate_node(child)?;
    }
    Ok(())
}

fn role_requires_label(role: AccessibilityRole) -> bool {
    role != AccessibilityRole::ScrollArea
}

fn role_requires_value(role: AccessibilityRole) -> bool {
    matches!(
        role,
        AccessibilityRole::Checkbox
            | AccessibilityRole::DataListItem
            | AccessibilityRole::DataTreeItem
            | AccessibilityRole::MenuItem
            | AccessibilityRole::ProgressIndicator
            | AccessibilityRole::SegmentedControl
            | AccessibilityRole::SegmentedOption
            | AccessibilityRole::Tab
            | AccessibilityRole::TabList
            | AccessibilityRole::TabPanel
            | AccessibilityRole::TextInput
    )
}

fn native_role(role: AccessibilityRole) -> &'static NSAccessibilityRole {
    unsafe {
        match role {
            AccessibilityRole::Button => NSAccessibilityButtonRole,
            AccessibilityRole::Checkbox => NSAccessibilityCheckBoxRole,
            AccessibilityRole::DataList => NSAccessibilityListRole,
            AccessibilityRole::DataListItem
            | AccessibilityRole::DataTableRow
            | AccessibilityRole::DataTreeItem => NSAccessibilityRowRole,
            AccessibilityRole::DataTableCell => NSAccessibilityCellRole,
            AccessibilityRole::DataTree => NSAccessibilityOutlineRole,
            AccessibilityRole::Dialog => NSAccessibilityGroupRole,
            AccessibilityRole::Menu => NSAccessibilityMenuRole,
            AccessibilityRole::MenuItem => NSAccessibilityMenuItemRole,
            AccessibilityRole::Popover
            | AccessibilityRole::SegmentedControl
            | AccessibilityRole::TabPanel => NSAccessibilityGroupRole,
            AccessibilityRole::ProgressIndicator => NSAccessibilityProgressIndicatorRole,
            AccessibilityRole::SegmentedOption | AccessibilityRole::Tab => {
                NSAccessibilityRadioButtonRole
            }
            AccessibilityRole::TabList => NSAccessibilityTabGroupRole,
            AccessibilityRole::Text => NSAccessibilityStaticTextRole,
            AccessibilityRole::TextInput => NSAccessibilityTextFieldRole,
            AccessibilityRole::ScrollArea => NSAccessibilityScrollAreaRole,
            AccessibilityRole::Toolbar => NSAccessibilityToolbarRole,
        }
    }
}

fn snapshot_node(node: &AccessibilityNode) -> MacAccessibilityNodeSnapshot {
    MacAccessibilityNodeSnapshot {
        id: node.a11y_id(),
        native_role: native_role_name(node.a11y_role()),
        label: node.a11y_label().map(str::to_string),
        value: node.a11y_value().map(str::to_string),
        enabled: node.a11y_enabled(),
        read_only: node.a11y_read_only(),
        invalid: node.a11y_invalid(),
        focused: node.a11y_focused(),
        selected: node.a11y_selected(),
        checked: node.a11y_checked(),
        native_actions: node
            .a11y_actions()
            .iter()
            .copied()
            .map(native_action_name)
            .collect(),
        children: node.a11y_children().iter().map(snapshot_node).collect(),
    }
}

fn native_role_name(role: AccessibilityRole) -> &'static str {
    match role {
        AccessibilityRole::Button => "AXButton",
        AccessibilityRole::Checkbox => "AXCheckBox",
        AccessibilityRole::DataList => "AXList",
        AccessibilityRole::DataListItem => "AXRow",
        AccessibilityRole::DataTableCell => "AXCell",
        AccessibilityRole::DataTableRow => "AXRow",
        AccessibilityRole::DataTree => "AXOutline",
        AccessibilityRole::DataTreeItem => "AXRow",
        AccessibilityRole::Dialog => "AXWindow",
        AccessibilityRole::Menu => "AXMenu",
        AccessibilityRole::MenuItem => "AXMenuItem",
        AccessibilityRole::Popover => "AXGroup",
        AccessibilityRole::ProgressIndicator => "AXProgressIndicator",
        AccessibilityRole::SegmentedControl => "AXGroup",
        AccessibilityRole::SegmentedOption => "AXRadioButton",
        AccessibilityRole::Tab => "AXRadioButton",
        AccessibilityRole::TabList => "AXTabGroup",
        AccessibilityRole::TabPanel => "AXGroup",
        AccessibilityRole::Text => "AXStaticText",
        AccessibilityRole::TextInput => "AXTextField",
        AccessibilityRole::ScrollArea => "AXScrollArea",
        AccessibilityRole::Toolbar => "AXToolbar",
    }
}

fn native_action_name(action: AccessibilityAction) -> &'static str {
    match action {
        AccessibilityAction::Activate => "AXPress",
        AccessibilityAction::SetValue => "AXSetValue",
        AccessibilityAction::ScrollForward => "AXScrollDown",
        AccessibilityAction::ScrollBackward => "AXScrollUp",
    }
}

fn native_notification_name(kind: AccessibilityAnnouncementKind) -> &'static str {
    match kind {
        AccessibilityAnnouncementKind::FocusChanged => "AXFocusedUIElementChanged",
        AccessibilityAnnouncementKind::ActionFeedback => "AXAnnouncementRequested",
    }
}
