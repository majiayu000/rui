use super::support::*;

// ==================== Complex Scenario Tests ====================

#[test]
fn test_password_input_scenario() {
    let focus_count = Rc::new(RefCell::new(0));
    let blur_count = Rc::new(RefCell::new(0));
    let focus_clone = focus_count.clone();
    let blur_clone = blur_count.clone();

    let inp = Input::new()
        .placeholder("Enter password")
        .password()
        .value("secret123")
        .on_focus(move || {
            *focus_clone.borrow_mut() += 1;
        })
        .on_blur(move || {
            *blur_clone.borrow_mut() += 1;
        });

    assert_eq!(inp.placeholder, "Enter password");
    assert_eq!(inp.state.value, "secret123");
    assert_eq!(
        inp.display_text(),
        "\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}\u{2022}"
    );

    // Simulate focus
    if let Some(handler) = &inp.on_focus {
        handler();
    }
    assert_eq!(*focus_count.borrow(), 1);

    // Simulate blur
    if let Some(handler) = &inp.on_blur {
        handler();
    }
    assert_eq!(*blur_count.borrow(), 1);
}

#[test]
fn test_search_input_scenario() {
    let search_results = Rc::new(RefCell::new(Vec::new()));
    let results_clone = search_results.clone();

    let inp = Input::new()
        .placeholder("Search...")
        .value("rust")
        .search()
        .w(300.0)
        .on_change(move |val| {
            results_clone.borrow_mut().push(val.to_string());
        });

    if let Some(handler) = &inp.on_change {
        handler(&inp.state.value);
    }
    assert_eq!(search_results.borrow().len(), 1);
    assert_eq!(search_results.borrow()[0], "rust");
}

#[test]
fn test_form_input_scenario() {
    let submitted = Rc::new(RefCell::new(false));
    let submitted_clone = submitted.clone();

    let inp = Input::new()
        .placeholder("Enter your email")
        .email()
        .value("user@example.com")
        .on_submit(move |_| {
            *submitted_clone.borrow_mut() = true;
        });

    if let Some(handler) = &inp.on_submit {
        handler(&inp.state.value);
    }
    assert!(*submitted.borrow());
}

#[test]
fn test_full_builder_chain() {
    let id = ElementId::new();
    let change_count = Rc::new(RefCell::new(0));
    let submit_count = Rc::new(RefCell::new(0));
    let focus_count = Rc::new(RefCell::new(0));
    let blur_count = Rc::new(RefCell::new(0));
    let change_clone = change_count.clone();
    let submit_clone = submit_count.clone();
    let focus_clone = focus_count.clone();
    let blur_clone = blur_count.clone();

    let inp = Input::new()
        .id(id)
        .value("initial")
        .placeholder("Type here")
        .password()
        .w(250.0)
        .rounded(8.0)
        .border_color(Color::BLUE)
        .on_change(move |_| {
            *change_clone.borrow_mut() += 1;
        })
        .on_submit(move |_| {
            *submit_clone.borrow_mut() += 1;
        })
        .on_focus(move || {
            *focus_clone.borrow_mut() += 1;
        })
        .on_blur(move || {
            *blur_clone.borrow_mut() += 1;
        });

    // Verify all properties
    assert_eq!(inp.id, Some(id));
    assert_eq!(inp.state.value, "initial");
    assert_eq!(inp.placeholder, "Type here");
    assert_eq!(inp.input_type, InputType::Password);
    assert_eq!(inp.width, Some(250.0));
    assert_eq!(inp.style.border.radius, Corners::all(8.0));
    assert_eq!(inp.style.border.color, Color::BLUE);
    assert!(inp.on_change.is_some());
    assert!(inp.on_submit.is_some());
    assert!(inp.on_focus.is_some());
    assert!(inp.on_blur.is_some());

    // Trigger all callbacks
    if let Some(handler) = &inp.on_change {
        handler(&inp.state.value);
    }
    if let Some(handler) = &inp.on_submit {
        handler(&inp.state.value);
    }
    if let Some(handler) = &inp.on_focus {
        handler();
    }
    if let Some(handler) = &inp.on_blur {
        handler();
    }

    assert_eq!(*change_count.borrow(), 1);
    assert_eq!(*submit_count.borrow(), 1);
    assert_eq!(*focus_count.borrow(), 1);
    assert_eq!(*blur_count.borrow(), 1);
}
