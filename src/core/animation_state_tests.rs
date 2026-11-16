use super::tests::approx_eq;
use super::*;
use crate::core::color::{Color, Rgba};
use crate::core::geometry::{Point, Size};
use std::f32::consts::PI;
use std::thread;
use std::time::Duration;

// ==================== AnimationState Tests ====================

#[test]
fn test_animation_state_variants() {
    let states = vec![
        AnimationState::Pending,
        AnimationState::Running,
        AnimationState::Paused,
        AnimationState::Completed,
    ];

    // Test that all states are distinct
    for (i, s1) in states.iter().enumerate() {
        for (j, s2) in states.iter().enumerate() {
            if i == j {
                assert_eq!(s1, s2);
            } else {
                assert_ne!(s1, s2);
            }
        }
    }
}

#[test]
fn test_animation_state_clone_copy() {
    let state = AnimationState::Running;
    let cloned = state.clone();
    let copied = state;

    assert_eq!(state, cloned);
    assert_eq!(state, copied);
}

// ==================== Animation Tests ====================

#[test]
fn test_animation_new() {
    let anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_secs(1));

    assert_eq!(anim.state(), AnimationState::Pending);
    assert!(!anim.is_running());
    assert!(!anim.is_completed());
}

#[test]
fn test_animation_builder_pattern() {
    let anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_secs(1))
        .easing(Easing::EaseInOut)
        .delay(Duration::from_millis(100));

    assert_eq!(anim.state(), AnimationState::Pending);
}

#[test]
fn test_animation_start() {
    let mut anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_secs(1));

    anim.start();

    assert_eq!(anim.state(), AnimationState::Running);
    assert!(anim.is_running());
    assert!(!anim.is_completed());
}

#[test]
fn test_animation_pause_resume() {
    let mut anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_secs(1));

    // Pause before start - should not change state
    anim.pause();
    assert_eq!(anim.state(), AnimationState::Pending);

    // Start and pause
    anim.start();
    anim.pause();
    assert_eq!(anim.state(), AnimationState::Paused);
    assert!(!anim.is_running());

    // Resume
    anim.resume();
    assert_eq!(anim.state(), AnimationState::Running);
    assert!(anim.is_running());

    // Resume when already running - no change
    anim.resume();
    assert_eq!(anim.state(), AnimationState::Running);
}

#[test]
fn test_animation_reset() {
    let mut anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_secs(1));

    anim.start();
    assert_eq!(anim.state(), AnimationState::Running);

    anim.reset();
    assert_eq!(anim.state(), AnimationState::Pending);
    assert!(!anim.is_running());
}

#[test]
fn test_animation_value_before_start() {
    let anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_secs(1));

    // Before start, value should be `from`
    let value = anim.value();
    assert!(approx_eq(value, 0.0, 0.001));
}

#[test]
fn test_animation_value_during_delay() {
    let mut anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_millis(100))
        .delay(Duration::from_millis(500));

    anim.start();

    // During delay, value should be `from`
    let value = anim.value();
    assert!(approx_eq(value, 0.0, 0.001));
}

#[test]
fn test_animation_value_with_zero_duration() {
    let mut anim = Animation::new(0.0_f32, 100.0_f32, Duration::ZERO);

    anim.start();
    thread::sleep(Duration::from_millis(10));

    // With zero duration, should immediately reach target
    let value = anim.value();
    assert!(approx_eq(value, 100.0, 0.001));
}

#[test]
fn test_animation_update_returns_false_when_not_running() {
    let mut anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_secs(1));

    // Not started
    assert!(!anim.update());

    // Paused
    anim.start();
    anim.pause();
    assert!(!anim.update());
}

#[test]
fn test_animation_update_returns_true_while_running() {
    let mut anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_millis(100));

    anim.start();
    assert!(anim.update());
}

#[test]
fn test_animation_completes_after_duration() {
    let mut anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_millis(50));

    anim.start();
    thread::sleep(Duration::from_millis(100));
    anim.update();

    assert_eq!(anim.state(), AnimationState::Completed);
    assert!(anim.is_completed());
    assert!(!anim.is_running());
}

#[test]
fn test_animation_on_complete_callback() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    let called = Arc::new(AtomicBool::new(false));
    let called_clone = called.clone();

    let mut anim =
        Animation::new(0.0_f32, 100.0_f32, Duration::from_millis(10)).on_complete(move || {
            called_clone.store(true, Ordering::SeqCst);
        });

    anim.start();
    thread::sleep(Duration::from_millis(50));
    anim.update();

    assert!(
        called.load(Ordering::SeqCst),
        "on_complete should be called"
    );
}

#[test]
fn test_animation_with_easing() {
    let mut anim =
        Animation::new(0.0_f32, 100.0_f32, Duration::from_millis(100)).easing(Easing::EaseIn);

    // Avoid sleep-based timing here; slower CI runners can overshoot the midpoint
    // enough to make the assertion flaky even when easing is correct.
    anim.state = AnimationState::Running;
    anim.start_time = Some(Instant::now() - Duration::from_millis(50));
    let value = anim.value();
    let expected_midpoint = 100.0 * Easing::EaseIn.apply(0.5);

    assert!(
        approx_eq(value, expected_midpoint, 5.0),
        "EaseIn animation should stay near the eased midpoint value, got {}",
        value
    );
}

// ==================== Animatable Trait Tests ====================

#[test]
fn test_animatable_f32() {
    let test_cases = vec![
        (0.0_f32, 100.0_f32, 0.0, 0.0),
        (0.0_f32, 100.0_f32, 0.5, 50.0),
        (0.0_f32, 100.0_f32, 1.0, 100.0),
        (-50.0_f32, 50.0_f32, 0.5, 0.0),
        (100.0_f32, 0.0_f32, 0.5, 50.0), // Reverse direction
    ];

    for (from, to, t, expected) in test_cases {
        let result = f32::interpolate(&from, &to, t);
        assert!(
            approx_eq(result, expected, 0.001),
            "f32::interpolate({}, {}, {}) = {}, expected {}",
            from,
            to,
            t,
            result,
            expected
        );
    }
}

#[test]
fn test_animatable_f64() {
    let test_cases = vec![
        (0.0_f64, 100.0_f64, 0.0, 0.0),
        (0.0_f64, 100.0_f64, 0.5, 50.0),
        (0.0_f64, 100.0_f64, 1.0, 100.0),
    ];

    for (from, to, t, expected) in test_cases {
        let result = f64::interpolate(&from, &to, t);
        assert!(
            (result - expected).abs() < 0.001,
            "f64::interpolate({}, {}, {}) = {}, expected {}",
            from,
            to,
            t,
            result,
            expected
        );
    }
}

#[test]
fn test_animatable_point() {
    let from = Point::new(0.0, 0.0);
    let to = Point::new(100.0, 200.0);

    let test_cases = vec![
        (0.0, Point::new(0.0, 0.0)),
        (0.5, Point::new(50.0, 100.0)),
        (1.0, Point::new(100.0, 200.0)),
    ];

    for (t, expected) in test_cases {
        let result = Point::interpolate(&from, &to, t);
        assert!(
            approx_eq(result.x, expected.x, 0.001) && approx_eq(result.y, expected.y, 0.001),
            "Point::interpolate at t={} = {:?}, expected {:?}",
            t,
            result,
            expected
        );
    }
}

#[test]
fn test_animatable_size() {
    let from = Size::new(100.0, 50.0);
    let to = Size::new(200.0, 100.0);

    let result = Size::interpolate(&from, &to, 0.5);
    assert!(approx_eq(result.width, 150.0, 0.001));
    assert!(approx_eq(result.height, 75.0, 0.001));
}

#[test]
fn test_animatable_rgba() {
    let from = Rgba::new(0.0, 0.0, 0.0, 1.0);
    let to = Rgba::new(1.0, 1.0, 1.0, 1.0);

    let result = Rgba::interpolate(&from, &to, 0.5);
    assert!(approx_eq(result.r, 0.5, 0.001));
    assert!(approx_eq(result.g, 0.5, 0.001));
    assert!(approx_eq(result.b, 0.5, 0.001));
    assert!(approx_eq(result.a, 1.0, 0.001));
}

#[test]
fn test_animatable_color() {
    let from = Color::rgba(0.0, 0.0, 0.0, 1.0);
    let to = Color::rgba(1.0, 1.0, 1.0, 1.0);

    let result = Color::interpolate(&from, &to, 0.5);
    let rgba = result.to_rgba();

    assert!(approx_eq(rgba.r, 0.5, 0.001));
    assert!(approx_eq(rgba.g, 0.5, 0.001));
    assert!(approx_eq(rgba.b, 0.5, 0.001));
}

#[test]
fn test_animatable_transform() {
    let from = Transform::identity();
    let to = Transform {
        translate_x: 100.0,
        translate_y: 200.0,
        scale_x: 2.0,
        scale_y: 2.0,
        rotation: PI,
        skew_x: 0.5,
        skew_y: 0.5,
    };

    let result = Transform::interpolate(&from, &to, 0.5);

    assert!(approx_eq(result.translate_x, 50.0, 0.001));
    assert!(approx_eq(result.translate_y, 100.0, 0.001));
    assert!(approx_eq(result.scale_x, 1.5, 0.001));
    assert!(approx_eq(result.scale_y, 1.5, 0.001));
    assert!(approx_eq(result.rotation, PI / 2.0, 0.001));
    assert!(approx_eq(result.skew_x, 0.25, 0.001));
    assert!(approx_eq(result.skew_y, 0.25, 0.001));
}

// ==================== Transform Tests ====================

#[test]
fn test_transform_identity() {
    let t = Transform::identity();

    assert!(approx_eq(t.translate_x, 0.0, 0.001));
    assert!(approx_eq(t.translate_y, 0.0, 0.001));
    assert!(approx_eq(t.scale_x, 1.0, 0.001));
    assert!(approx_eq(t.scale_y, 1.0, 0.001));
    assert!(approx_eq(t.rotation, 0.0, 0.001));
    assert!(approx_eq(t.skew_x, 0.0, 0.001));
    assert!(approx_eq(t.skew_y, 0.0, 0.001));
}

#[test]
fn test_transform_default() {
    let t = Transform::default();

    // Default should have scale 0, not 1 (unlike identity)
    assert!(approx_eq(t.translate_x, 0.0, 0.001));
    assert!(approx_eq(t.translate_y, 0.0, 0.001));
    assert!(approx_eq(t.scale_x, 0.0, 0.001));
    assert!(approx_eq(t.scale_y, 0.0, 0.001));
}

#[test]
fn test_transform_translate() {
    let t = Transform::translate(10.0, 20.0);

    assert!(approx_eq(t.translate_x, 10.0, 0.001));
    assert!(approx_eq(t.translate_y, 20.0, 0.001));
    assert!(approx_eq(t.scale_x, 1.0, 0.001));
    assert!(approx_eq(t.scale_y, 1.0, 0.001));
}

#[test]
fn test_transform_scale() {
    let t = Transform::scale(2.0, 3.0);

    assert!(approx_eq(t.scale_x, 2.0, 0.001));
    assert!(approx_eq(t.scale_y, 3.0, 0.001));
    assert!(approx_eq(t.translate_x, 0.0, 0.001));
    assert!(approx_eq(t.translate_y, 0.0, 0.001));
}

#[test]
fn test_transform_rotate() {
    let t = Transform::rotate(PI / 2.0);

    assert!(approx_eq(t.rotation, PI / 2.0, 0.001));
    assert!(approx_eq(t.scale_x, 1.0, 0.001));
}

#[test]
fn test_transform_rotate_deg() {
    let t = Transform::rotate_deg(90.0);

    assert!(approx_eq(t.rotation, PI / 2.0, 0.001));
}

#[test]
fn test_transform_then() {
    let t1 = Transform::translate(10.0, 20.0);
    let t2 = Transform::scale(2.0, 2.0);

    let combined = t1.then(t2);

    // Translation should add
    assert!(approx_eq(combined.translate_x, 10.0, 0.001));
    assert!(approx_eq(combined.translate_y, 20.0, 0.001));
    // Scale should multiply
    assert!(approx_eq(combined.scale_x, 2.0, 0.001));
    assert!(approx_eq(combined.scale_y, 2.0, 0.001));
}

#[test]
fn test_transform_then_with_rotation() {
    let t1 = Transform::rotate(PI / 4.0);
    let t2 = Transform::rotate(PI / 4.0);

    let combined = t1.then(t2);

    // Rotation should add
    assert!(approx_eq(combined.rotation, PI / 2.0, 0.001));
}

#[test]
fn test_transform_to_matrix() {
    let identity = Transform::identity();
    let matrix = identity.to_matrix();

    // Identity matrix should be [[1,0,0], [0,1,0], [0,0,1]]
    assert!(approx_eq(matrix[0][0], 1.0, 0.001));
    assert!(approx_eq(matrix[0][1], 0.0, 0.001));
    assert!(approx_eq(matrix[0][2], 0.0, 0.001));
    assert!(approx_eq(matrix[1][0], 0.0, 0.001));
    assert!(approx_eq(matrix[1][1], 1.0, 0.001));
    assert!(approx_eq(matrix[1][2], 0.0, 0.001));
    assert!(approx_eq(matrix[2][0], 0.0, 0.001));
    assert!(approx_eq(matrix[2][1], 0.0, 0.001));
    assert!(approx_eq(matrix[2][2], 1.0, 0.001));
}

#[test]
fn test_transform_to_matrix_with_translation() {
    let t = Transform::translate(5.0, 10.0);
    let matrix = t.to_matrix();

    assert!(approx_eq(matrix[0][2], 5.0, 0.001));
    assert!(approx_eq(matrix[1][2], 10.0, 0.001));
}

#[test]
fn test_transform_to_matrix_with_scale() {
    let t = Transform::scale(2.0, 3.0);
    let matrix = t.to_matrix();

    assert!(approx_eq(matrix[0][0], 2.0, 0.001));
    assert!(approx_eq(matrix[1][1], 3.0, 0.001));
}

#[test]
fn test_transform_to_matrix_with_rotation() {
    let t = Transform::rotate(PI / 2.0);
    let matrix = t.to_matrix();

    // cos(90deg) = 0, sin(90deg) = 1
    assert!(approx_eq(matrix[0][0], 0.0, 0.001)); // cos
    assert!(approx_eq(matrix[0][1], -1.0, 0.001)); // -sin
    assert!(approx_eq(matrix[1][0], 1.0, 0.001)); // sin
    assert!(approx_eq(matrix[1][1], 0.0, 0.001)); // cos
}

// ==================== Transition Tests ====================

#[test]
fn test_transition_new() {
    let transition = Transition::new(TransitionProperty::Opacity, Duration::from_millis(300));

    assert_eq!(transition.property, TransitionProperty::Opacity);
    assert_eq!(transition.duration, Duration::from_millis(300));
    assert_eq!(transition.easing, Easing::EaseInOut);
    assert_eq!(transition.delay, Duration::ZERO);
}

#[test]
fn test_transition_all() {
    let transition = Transition::all(Duration::from_millis(500));

    assert_eq!(transition.property, TransitionProperty::All);
    assert_eq!(transition.duration, Duration::from_millis(500));
}

#[test]
fn test_transition_builder() {
    let transition = Transition::new(TransitionProperty::Background, Duration::from_millis(200))
        .easing(Easing::EaseOut)
        .delay(Duration::from_millis(100));

    assert_eq!(transition.property, TransitionProperty::Background);
    assert_eq!(transition.easing, Easing::EaseOut);
    assert_eq!(transition.delay, Duration::from_millis(100));
}

// ==================== TransitionProperty Tests ====================

#[test]
fn test_transition_property_variants() {
    let properties = vec![
        TransitionProperty::All,
        TransitionProperty::Opacity,
        TransitionProperty::Background,
        TransitionProperty::BorderColor,
        TransitionProperty::Transform,
        TransitionProperty::Width,
        TransitionProperty::Height,
        TransitionProperty::Padding,
        TransitionProperty::Margin,
    ];

    // Test that all variants are distinct
    for (i, p1) in properties.iter().enumerate() {
        for (j, p2) in properties.iter().enumerate() {
            if i == j {
                assert_eq!(p1, p2);
            } else {
                assert_ne!(p1, p2);
            }
        }
    }
}

#[test]
fn test_transition_property_clone_copy() {
    let prop = TransitionProperty::Opacity;
    let cloned = prop.clone();
    let copied = prop;

    assert_eq!(prop, cloned);
    assert_eq!(prop, copied);
}

// ==================== Edge Case Tests ====================

#[test]
fn test_animation_with_negative_values() {
    let mut anim = Animation::new(-100.0_f32, -50.0_f32, Duration::from_millis(50));

    anim.start();
    thread::sleep(Duration::from_millis(60));
    anim.update();

    let value = anim.value();
    assert!(approx_eq(value, -50.0, 0.001));
}

#[test]
fn test_animation_from_equals_to() {
    let mut anim = Animation::new(50.0_f32, 50.0_f32, Duration::from_millis(50));

    anim.start();
    let value = anim.value();
    assert!(approx_eq(value, 50.0, 0.001));

    thread::sleep(Duration::from_millis(60));
    anim.update();
    let value = anim.value();
    assert!(approx_eq(value, 50.0, 0.001));
}

#[test]
fn test_multiple_easing_functions_same_animation() {
    // Test chaining easing changes
    let anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_secs(1))
        .easing(Easing::EaseIn)
        .easing(Easing::EaseOut)
        .easing(Easing::EaseInOut);

    // Last easing should win
    assert_eq!(anim.state(), AnimationState::Pending);
}

#[test]
fn test_animation_pause_does_not_affect_non_running() {
    let mut anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_secs(1));

    // Pause when pending - should stay pending
    anim.pause();
    assert_eq!(anim.state(), AnimationState::Pending);

    // Start and complete
    anim.start();
    thread::sleep(Duration::from_millis(10));

    // Simulate completion by calling reset, then start with very short duration
    let mut anim2 = Animation::new(0.0_f32, 100.0_f32, Duration::from_millis(1));
    anim2.start();
    thread::sleep(Duration::from_millis(10));
    anim2.update();

    // Now pausing completed animation should not change state
    assert_eq!(anim2.state(), AnimationState::Completed);
    anim2.pause();
    assert_eq!(anim2.state(), AnimationState::Completed);
}

#[test]
fn test_animation_resume_does_not_affect_non_paused() {
    let mut anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_secs(1));

    // Resume when pending - should stay pending
    anim.resume();
    assert_eq!(anim.state(), AnimationState::Pending);

    // Start then resume - should stay running
    anim.start();
    anim.resume();
    assert_eq!(anim.state(), AnimationState::Running);
}

// ==================== Integration Tests ====================

#[test]
fn test_full_animation_lifecycle() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU32, Ordering};

    let counter = Arc::new(AtomicU32::new(0));
    let counter_clone = counter.clone();

    let mut anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_millis(30))
        .easing(Easing::Linear)
        .delay(Duration::from_millis(10))
        .on_complete(move || {
            counter_clone.fetch_add(1, Ordering::SeqCst);
        });

    // Initial state
    assert_eq!(anim.state(), AnimationState::Pending);
    assert!(approx_eq(anim.value(), 0.0, 0.001));

    // Start
    anim.start();
    assert_eq!(anim.state(), AnimationState::Running);

    // During delay
    let value_during_delay = anim.value();
    assert!(approx_eq(value_during_delay, 0.0, 0.001));

    // Pause/Resume
    anim.pause();
    assert_eq!(anim.state(), AnimationState::Paused);
    anim.resume();
    assert_eq!(anim.state(), AnimationState::Running);

    // Wait for completion
    thread::sleep(Duration::from_millis(100));
    let still_running = anim.update();
    assert!(!still_running);
    assert_eq!(anim.state(), AnimationState::Completed);
    assert!(approx_eq(anim.value(), 100.0, 0.001));

    // Callback should have been called
    assert_eq!(counter.load(Ordering::SeqCst), 1);

    // Reset
    anim.reset();
    assert_eq!(anim.state(), AnimationState::Pending);
}

#[test]
fn test_animation_with_all_easing_functions() {
    let easings = vec![
        Easing::Linear,
        Easing::EaseIn,
        Easing::EaseOut,
        Easing::EaseInOut,
        Easing::EaseInQuad,
        Easing::EaseOutQuad,
        Easing::EaseInOutQuad,
        Easing::EaseInCubic,
        Easing::EaseOutCubic,
        Easing::EaseInOutCubic,
        Easing::EaseInQuart,
        Easing::EaseOutQuart,
        Easing::EaseInOutQuart,
        Easing::EaseInExpo,
        Easing::EaseOutExpo,
        Easing::EaseInOutExpo,
        Easing::EaseInBack,
        Easing::EaseOutBack,
        Easing::EaseInOutBack,
        Easing::EaseInElastic,
        Easing::EaseOutElastic,
        Easing::EaseInOutElastic,
        Easing::EaseInBounce,
        Easing::EaseOutBounce,
        Easing::EaseInOutBounce,
        Easing::Spring {
            stiffness: 100.0,
            damping: 10.0,
        },
        Easing::Custom(|t| t),
    ];

    for easing in easings {
        let mut anim =
            Animation::new(0.0_f32, 100.0_f32, Duration::from_millis(20)).easing(easing.clone());

        anim.start();
        thread::sleep(Duration::from_millis(30));
        anim.update();

        let final_value = anim.value();
        assert!(
            approx_eq(final_value, 100.0, 0.001),
            "Animation with easing {:?} should end at 100, got {}",
            easing,
            final_value
        );
    }
}
