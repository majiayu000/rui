use super::support::*;

// ==================== on_change Callback Tests ====================

#[test]
fn test_on_change_not_set() {
    let inp = Input::new();
    assert!(inp.on_change.is_none());
}

#[test]
fn test_on_change_set() {
    let inp = Input::new().on_change(|_| {});
    assert!(inp.on_change.is_some());
}

#[test]
fn test_on_change_callback_called() {
    let called = Rc::new(RefCell::new(false));
    let called_clone = called.clone();

    let inp = Input::new().value("test").on_change(move |_| {
        *called_clone.borrow_mut() = true;
    });

    // Simulate calling the handler
    if let Some(handler) = &inp.on_change {
        handler(&inp.state.value);
    }
    assert!(*called.borrow());
}

#[test]
fn test_on_change_receives_value() {
    let received_value = Rc::new(RefCell::new(String::new()));
    let received_clone = received_value.clone();

    let inp = Input::new().value("hello world").on_change(move |val| {
        *received_clone.borrow_mut() = val.to_string();
    });

    if let Some(handler) = &inp.on_change {
        handler(&inp.state.value);
    }
    assert_eq!(*received_value.borrow(), "hello world");
}

// ==================== on_submit Callback Tests ====================

#[test]
fn test_on_submit_not_set() {
    let inp = Input::new();
    assert!(inp.on_submit.is_none());
}

#[test]
fn test_on_submit_set() {
    let inp = Input::new().on_submit(|_| {});
    assert!(inp.on_submit.is_some());
}

#[test]
fn test_on_submit_callback_called() {
    let called = Rc::new(RefCell::new(false));
    let called_clone = called.clone();

    let inp = Input::new().value("test").on_submit(move |_| {
        *called_clone.borrow_mut() = true;
    });

    if let Some(handler) = &inp.on_submit {
        handler(&inp.state.value);
    }
    assert!(*called.borrow());
}

#[test]
fn test_on_submit_receives_value() {
    let received_value = Rc::new(RefCell::new(String::new()));
    let received_clone = received_value.clone();

    let inp = Input::new().value("submitted text").on_submit(move |val| {
        *received_clone.borrow_mut() = val.to_string();
    });

    if let Some(handler) = &inp.on_submit {
        handler(&inp.state.value);
    }
    assert_eq!(*received_value.borrow(), "submitted text");
}

// ==================== on_cancel Callback Tests ====================

#[test]
fn test_on_cancel_not_set() {
    let inp = Input::new();
    assert!(inp.on_cancel.is_none());
}

#[test]
fn test_on_cancel_set() {
    let inp = Input::new().on_cancel(|| {});
    assert!(inp.on_cancel.is_some());
}

#[test]
fn test_on_cancel_callback_called_by_escape_key() {
    let called = Rc::new(RefCell::new(false));
    let called_clone = called.clone();

    let mut inp = Input::new().value("draft").on_cancel(move || {
        *called_clone.borrow_mut() = true;
    });

    let outcome = inp
        .apply_key_event(&key_event(KeyCode::Escape))
        .expect("escape cancel should be handled");

    assert!(!outcome.changed);
    assert!(!outcome.submitted);
    assert_eq!(inp.state.value, "draft");
    assert!(*called.borrow());
}

// ==================== on_focus Callback Tests ====================

#[test]
fn test_on_focus_not_set() {
    let inp = Input::new();
    assert!(inp.on_focus.is_none());
}

#[test]
fn test_on_focus_set() {
    let inp = Input::new().on_focus(|| {});
    assert!(inp.on_focus.is_some());
}

#[test]
fn test_on_focus_callback_called() {
    let called = Rc::new(RefCell::new(false));
    let called_clone = called.clone();

    let inp = Input::new().on_focus(move || {
        *called_clone.borrow_mut() = true;
    });

    if let Some(handler) = &inp.on_focus {
        handler();
    }
    assert!(*called.borrow());
}

// ==================== on_blur Callback Tests ====================

#[test]
fn test_on_blur_not_set() {
    let inp = Input::new();
    assert!(inp.on_blur.is_none());
}

#[test]
fn test_on_blur_set() {
    let inp = Input::new().on_blur(|| {});
    assert!(inp.on_blur.is_some());
}

#[test]
fn test_on_blur_callback_called() {
    let called = Rc::new(RefCell::new(false));
    let called_clone = called.clone();

    let inp = Input::new().on_blur(move || {
        *called_clone.borrow_mut() = true;
    });

    if let Some(handler) = &inp.on_blur {
        handler();
    }
    assert!(*called.borrow());
}
