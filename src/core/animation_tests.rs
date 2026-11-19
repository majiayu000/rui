use super::*;

// ==================== Easing Function Tests ====================

/// Helper to compare floats with tolerance
pub(super) fn approx_eq(a: f32, b: f32, tolerance: f32) -> bool {
    (a - b).abs() < tolerance
}

#[test]
fn test_easing_boundary_values() {
    // All easing functions should return 0 at t=0 and 1 at t=1
    let test_cases = vec![
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
    ];

    for easing in test_cases {
        let at_zero = easing.apply(0.0);
        let at_one = easing.apply(1.0);

        assert!(
            approx_eq(at_zero, 0.0, 0.001),
            "Easing {:?} at t=0 should be ~0, got {}",
            easing,
            at_zero
        );
        assert!(
            approx_eq(at_one, 1.0, 0.001),
            "Easing {:?} at t=1 should be ~1, got {}",
            easing,
            at_one
        );
    }
}

#[test]
fn test_easing_linear() {
    let test_cases = vec![
        (0.0, 0.0),
        (0.25, 0.25),
        (0.5, 0.5),
        (0.75, 0.75),
        (1.0, 1.0),
    ];

    for (t, expected) in test_cases {
        let result = Easing::Linear.apply(t);
        assert!(
            approx_eq(result, expected, 0.001),
            "Linear({}) = {}, expected {}",
            t,
            result,
            expected
        );
    }
}

#[test]
fn test_easing_ease_in() {
    // EaseIn should be slower at the start (below linear)
    let test_cases = vec![
        (0.0, 0.0),
        (0.25, 0.0625), // 0.25^2
        (0.5, 0.25),    // 0.5^2
        (0.75, 0.5625), // 0.75^2
        (1.0, 1.0),
    ];

    for (t, expected) in test_cases {
        let result = Easing::EaseIn.apply(t);
        assert!(
            approx_eq(result, expected, 0.001),
            "EaseIn({}) = {}, expected {}",
            t,
            result,
            expected
        );
    }
}

#[test]
fn test_easing_ease_out() {
    // EaseOut should be faster at the start (above linear)
    let result_quarter = Easing::EaseOut.apply(0.25);
    let result_half = Easing::EaseOut.apply(0.5);
    let result_three_quarter = Easing::EaseOut.apply(0.75);

    // EaseOut values should be above linear values
    assert!(result_quarter > 0.25, "EaseOut(0.25) should be > 0.25");
    assert!(result_half > 0.5, "EaseOut(0.5) should be > 0.5");
    assert!(
        result_three_quarter > 0.75,
        "EaseOut(0.75) should be > 0.75"
    );
}

#[test]
fn test_easing_ease_in_out() {
    // EaseInOut: slow at start, fast in middle, slow at end
    let at_quarter = Easing::EaseInOut.apply(0.25);
    let at_half = Easing::EaseInOut.apply(0.5);
    let at_three_quarter = Easing::EaseInOut.apply(0.75);

    // First half should be below 0.5, second half should be above 0.5
    assert!(at_quarter < 0.25, "EaseInOut(0.25) should be < 0.25");
    assert!(
        approx_eq(at_half, 0.5, 0.001),
        "EaseInOut(0.5) should be ~0.5"
    );
    assert!(at_three_quarter > 0.75, "EaseInOut(0.75) should be > 0.75");
}

#[test]
fn test_easing_quad_variants() {
    // Quad = quadratic = t^2
    let test_cases = vec![
        (Easing::EaseInQuad, 0.5, 0.25),  // 0.5^2
        (Easing::EaseOutQuad, 0.5, 0.75), // 1 - (1-0.5)^2
    ];

    for (easing, t, expected) in test_cases {
        let result = easing.apply(t);
        assert!(
            approx_eq(result, expected, 0.001),
            "{:?}({}) = {}, expected {}",
            easing,
            t,
            result,
            expected
        );
    }
}

#[test]
fn test_easing_cubic_variants() {
    // Cubic = t^3
    let test_cases = vec![
        (Easing::EaseInCubic, 0.5, 0.125),  // 0.5^3
        (Easing::EaseOutCubic, 0.5, 0.875), // 1 - (1-0.5)^3
    ];

    for (easing, t, expected) in test_cases {
        let result = easing.apply(t);
        assert!(
            approx_eq(result, expected, 0.001),
            "{:?}({}) = {}, expected {}",
            easing,
            t,
            result,
            expected
        );
    }
}

#[test]
fn test_easing_quart_variants() {
    // Quart = t^4
    let test_cases = vec![
        (Easing::EaseInQuart, 0.5, 0.0625),  // 0.5^4
        (Easing::EaseOutQuart, 0.5, 0.9375), // 1 - (1-0.5)^4
    ];

    for (easing, t, expected) in test_cases {
        let result = easing.apply(t);
        assert!(
            approx_eq(result, expected, 0.001),
            "{:?}({}) = {}, expected {}",
            easing,
            t,
            result,
            expected
        );
    }
}

#[test]
fn test_easing_expo_variants() {
    // Test expo boundary conditions
    assert!(approx_eq(Easing::EaseInExpo.apply(0.0), 0.0, 0.001));
    assert!(approx_eq(Easing::EaseOutExpo.apply(1.0), 1.0, 0.001));

    // EaseInExpo should be very slow at start
    let ease_in_expo_quarter = Easing::EaseInExpo.apply(0.25);
    assert!(
        ease_in_expo_quarter < 0.1,
        "EaseInExpo(0.25) should be < 0.1, got {}",
        ease_in_expo_quarter
    );

    // EaseOutExpo should be very fast at start
    let ease_out_expo_quarter = Easing::EaseOutExpo.apply(0.25);
    assert!(
        ease_out_expo_quarter > 0.5,
        "EaseOutExpo(0.25) should be > 0.5, got {}",
        ease_out_expo_quarter
    );
}

#[test]
fn test_easing_back_overshoot() {
    // Back easing overshoots the target
    // EaseInBack goes negative initially
    let at_start = Easing::EaseInBack.apply(0.1);
    assert!(at_start < 0.0, "EaseInBack should go negative at start");

    let out_at_end = Easing::EaseOutBack.apply(0.9);
    // EaseOutBack overshoots 1.0
    assert!(
        out_at_end > 1.0,
        "EaseOutBack should overshoot 1.0 near end"
    );
}

#[test]
fn test_easing_elastic_oscillation() {
    // Elastic should oscillate around target
    // At t=0.5, it should be somewhere in oscillation
    let elastic_out_mid = Easing::EaseOutElastic.apply(0.5);
    // Just verify it's in a reasonable range (elastic can overshoot)
    assert!(
        elastic_out_mid > 0.5 && elastic_out_mid < 1.5,
        "EaseOutElastic(0.5) should be in reasonable range"
    );
}

#[test]
fn test_easing_bounce() {
    // Bounce mimics a bouncing ball
    let bounce_out = Easing::EaseOutBounce.apply(0.5);
    // Should be above linear
    assert!(
        bounce_out > 0.5,
        "EaseOutBounce(0.5) should be > 0.5, got {}",
        bounce_out
    );

    // Test multiple bounce regions
    let test_points = vec![0.1, 0.3, 0.5, 0.7, 0.9];
    for t in test_points {
        let result = Easing::EaseOutBounce.apply(t);
        assert!(
            (0.0..=1.1).contains(&result),
            "EaseOutBounce({}) = {} should be in [0, 1.1]",
            t,
            result
        );
    }
}

#[test]
fn test_easing_spring() {
    // Test underdamped spring (oscillates)
    let spring_underdamped = Easing::Spring {
        stiffness: 100.0,
        damping: 10.0,
    };
    let result = spring_underdamped.apply(0.5);
    // Just verify it produces a value
    assert!(result.is_finite(), "Spring should produce finite values");

    // Test critically damped spring
    let spring_critical = Easing::Spring {
        stiffness: 100.0,
        damping: 20.0,
    };
    let result = spring_critical.apply(0.5);
    assert!(
        result.is_finite(),
        "Critically damped spring should produce finite values"
    );
}

#[test]
fn test_easing_custom() {
    // Custom easing function
    fn custom_ease(t: f32) -> f32 {
        t * t * t // Cubic
    }

    let easing = Easing::Custom(custom_ease);
    let test_cases = vec![
        (0.0, 0.0),
        (0.5, 0.125), // 0.5^3
        (1.0, 1.0),
    ];

    for (t, expected) in test_cases {
        let result = easing.apply(t);
        assert!(
            approx_eq(result, expected, 0.001),
            "Custom({}) = {}, expected {}",
            t,
            result,
            expected
        );
    }
}

#[test]
fn test_easing_clamping() {
    // Values outside [0, 1] should be clamped
    let test_cases = vec![Easing::Linear, Easing::EaseIn, Easing::EaseOut];

    for easing in test_cases {
        let below = easing.apply(-0.5);
        let above = easing.apply(1.5);

        assert!(
            approx_eq(below, easing.apply(0.0), 0.001),
            "Easing {:?} should clamp negative values",
            easing
        );
        assert!(
            approx_eq(above, easing.apply(1.0), 0.001),
            "Easing {:?} should clamp values > 1",
            easing
        );
    }
}

#[test]
fn test_easing_default() {
    let default = Easing::default();
    assert_eq!(default, Easing::Linear);
}

#[test]
fn test_easing_clone_and_copy() {
    let easing = Easing::EaseInOut;
    let cloned = easing;
    let copied = easing;

    assert_eq!(easing, cloned);
    assert_eq!(easing, copied);
}
