//! Animation system for smooth UI transitions

use std::time::{Duration, Instant};

/// Easing functions for animations
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Easing {
    #[default]
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    EaseInQuart,
    EaseOutQuart,
    EaseInOutQuart,
    EaseInExpo,
    EaseOutExpo,
    EaseInOutExpo,
    EaseInBack,
    EaseOutBack,
    EaseInOutBack,
    EaseInElastic,
    EaseOutElastic,
    EaseInOutElastic,
    EaseInBounce,
    EaseOutBounce,
    EaseInOutBounce,
    Spring { stiffness: f32, damping: f32 },
    Custom(fn(f32) -> f32),
}

impl Easing {
    /// Apply the easing function to a value t in [0, 1]
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);

        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Easing::EaseInQuad => t * t,
            Easing::EaseOutQuad => 1.0 - (1.0 - t).powi(2),
            Easing::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Easing::EaseInCubic => t * t * t,
            Easing::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            Easing::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
            Easing::EaseInQuart => t * t * t * t,
            Easing::EaseOutQuart => 1.0 - (1.0 - t).powi(4),
            Easing::EaseInOutQuart => {
                if t < 0.5 {
                    8.0 * t * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(4) / 2.0
                }
            }
            Easing::EaseInExpo => {
                if t == 0.0 { 0.0 } else { 2.0_f32.powf(10.0 * t - 10.0) }
            }
            Easing::EaseOutExpo => {
                if t == 1.0 { 1.0 } else { 1.0 - 2.0_f32.powf(-10.0 * t) }
            }
            Easing::EaseInOutExpo => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else if t < 0.5 {
                    2.0_f32.powf(20.0 * t - 10.0) / 2.0
                } else {
                    (2.0 - 2.0_f32.powf(-20.0 * t + 10.0)) / 2.0
                }
            }
            Easing::EaseInBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                c3 * t * t * t - c1 * t * t
            }
            Easing::EaseOutBack => {
                let c1 = 1.70158;
                let c3 = c1 + 1.0;
                1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
            }
            Easing::EaseInOutBack => {
                let c1 = 1.70158;
                let c2 = c1 * 1.525;
                if t < 0.5 {
                    ((2.0 * t).powi(2) * ((c2 + 1.0) * 2.0 * t - c2)) / 2.0
                } else {
                    ((2.0 * t - 2.0).powi(2) * ((c2 + 1.0) * (t * 2.0 - 2.0) + c2) + 2.0) / 2.0
                }
            }
            Easing::EaseInElastic => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                    -2.0_f32.powf(10.0 * t - 10.0) * ((t * 10.0 - 10.75) * c4).sin()
                }
            }
            Easing::EaseOutElastic => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    let c4 = (2.0 * std::f32::consts::PI) / 3.0;
                    2.0_f32.powf(-10.0 * t) * ((t * 10.0 - 0.75) * c4).sin() + 1.0
                }
            }
            Easing::EaseInOutElastic => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    let c5 = (2.0 * std::f32::consts::PI) / 4.5;
                    if t < 0.5 {
                        -(2.0_f32.powf(20.0 * t - 10.0) * ((20.0 * t - 11.125) * c5).sin()) / 2.0
                    } else {
                        (2.0_f32.powf(-20.0 * t + 10.0) * ((20.0 * t - 11.125) * c5).sin()) / 2.0 + 1.0
                    }
                }
            }
            Easing::EaseInBounce => 1.0 - Easing::EaseOutBounce.apply(1.0 - t),
            Easing::EaseOutBounce => {
                let n1 = 7.5625;
                let d1 = 2.75;
                if t < 1.0 / d1 {
                    n1 * t * t
                } else if t < 2.0 / d1 {
                    let t = t - 1.5 / d1;
                    n1 * t * t + 0.75
                } else if t < 2.5 / d1 {
                    let t = t - 2.25 / d1;
                    n1 * t * t + 0.9375
                } else {
                    let t = t - 2.625 / d1;
                    n1 * t * t + 0.984375
                }
            }
            Easing::EaseInOutBounce => {
                if t < 0.5 {
                    (1.0 - Easing::EaseOutBounce.apply(1.0 - 2.0 * t)) / 2.0
                } else {
                    (1.0 + Easing::EaseOutBounce.apply(2.0 * t - 1.0)) / 2.0
                }
            }
            Easing::Spring { stiffness, damping } => {
                // Simplified spring physics
                let omega = stiffness.sqrt();
                let zeta = *damping / (2.0 * omega);
                if zeta < 1.0 {
                    // Underdamped
                    let omega_d = omega * (1.0 - zeta * zeta).sqrt();
                    1.0 - (-zeta * omega * t).exp() * ((zeta * omega * t / omega_d).cos() + zeta * (zeta * omega * t / omega_d).sin())
                } else {
                    // Critically damped or overdamped
                    1.0 - (1.0 + omega * t) * (-omega * t).exp()
                }
            }
            Easing::Custom(f) => f(t),
        }
    }
}

/// Animation state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationState {
    Pending,
    Running,
    Paused,
    Completed,
}

/// A single animation
pub struct Animation<T: Animatable> {
    from: T,
    to: T,
    duration: Duration,
    easing: Easing,
    delay: Duration,
    start_time: Option<Instant>,
    state: AnimationState,
    on_complete: Option<Box<dyn Fn()>>,
}

impl<T: Animatable> Animation<T> {
    pub fn new(from: T, to: T, duration: Duration) -> Self {
        Self {
            from,
            to,
            duration,
            easing: Easing::default(),
            delay: Duration::ZERO,
            start_time: None,
            state: AnimationState::Pending,
            on_complete: None,
        }
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    pub fn on_complete(mut self, f: impl Fn() + 'static) -> Self {
        self.on_complete = Some(Box::new(f));
        self
    }

    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
        self.state = AnimationState::Running;
    }

    pub fn pause(&mut self) {
        if self.state == AnimationState::Running {
            self.state = AnimationState::Paused;
        }
    }

    pub fn resume(&mut self) {
        if self.state == AnimationState::Paused {
            self.state = AnimationState::Running;
        }
    }

    pub fn reset(&mut self) {
        self.start_time = None;
        self.state = AnimationState::Pending;
    }

    pub fn state(&self) -> AnimationState {
        self.state
    }

    pub fn is_running(&self) -> bool {
        self.state == AnimationState::Running
    }

    pub fn is_completed(&self) -> bool {
        self.state == AnimationState::Completed
    }

    /// Get the current animated value
    pub fn value(&self) -> T {
        let Some(start_time) = self.start_time else {
            return self.from.clone();
        };

        let elapsed = start_time.elapsed();

        // Handle delay
        if elapsed < self.delay {
            return self.from.clone();
        }

        let elapsed = elapsed - self.delay;

        // Calculate progress
        let progress = if self.duration.as_secs_f32() > 0.0 {
            elapsed.as_secs_f32() / self.duration.as_secs_f32()
        } else {
            1.0
        };

        if progress >= 1.0 {
            return self.to.clone();
        }

        let t = self.easing.apply(progress);
        T::interpolate(&self.from, &self.to, t)
    }

    /// Update the animation and return true if still running
    pub fn update(&mut self) -> bool {
        if self.state != AnimationState::Running {
            return false;
        }

        let Some(start_time) = self.start_time else {
            return false;
        };

        let elapsed = start_time.elapsed();
        let total_duration = self.delay + self.duration;

        if elapsed >= total_duration {
            self.state = AnimationState::Completed;
            if let Some(ref on_complete) = self.on_complete {
                on_complete();
            }
            return false;
        }

        true
    }
}

/// Trait for types that can be animated
pub trait Animatable: Clone {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self;
}

impl Animatable for f32 {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self {
        from + (to - from) * t
    }
}

impl Animatable for f64 {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self {
        from + (to - from) * t as f64
    }
}

impl Animatable for crate::core::geometry::Point {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self {
        Self {
            x: f32::interpolate(&from.x, &to.x, t),
            y: f32::interpolate(&from.y, &to.y, t),
        }
    }
}

impl Animatable for crate::core::geometry::Size {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self {
        Self {
            width: f32::interpolate(&from.width, &to.width, t),
            height: f32::interpolate(&from.height, &to.height, t),
        }
    }
}

impl Animatable for crate::core::color::Rgba {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self {
        Self {
            r: f32::interpolate(&from.r, &to.r, t),
            g: f32::interpolate(&from.g, &to.g, t),
            b: f32::interpolate(&from.b, &to.b, t),
            a: f32::interpolate(&from.a, &to.a, t),
        }
    }
}

impl Animatable for crate::core::color::Color {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self {
        let from_rgba = from.to_rgba();
        let to_rgba = to.to_rgba();
        crate::core::color::Color::Rgba(crate::core::color::Rgba::interpolate(&from_rgba, &to_rgba, t))
    }
}

/// Transition helper for animating style changes
#[derive(Clone)]
pub struct Transition {
    pub property: TransitionProperty,
    pub duration: Duration,
    pub easing: Easing,
    pub delay: Duration,
}

impl Transition {
    pub fn new(property: TransitionProperty, duration: Duration) -> Self {
        Self {
            property,
            duration,
            easing: Easing::EaseInOut,
            delay: Duration::ZERO,
        }
    }

    pub fn all(duration: Duration) -> Self {
        Self::new(TransitionProperty::All, duration)
    }

    pub fn easing(mut self, easing: Easing) -> Self {
        self.easing = easing;
        self
    }

    pub fn delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
}

/// Properties that can be transitioned
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionProperty {
    All,
    Opacity,
    Background,
    BorderColor,
    Transform,
    Width,
    Height,
    Padding,
    Margin,
}

/// Transform operations for elements
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Transform {
    pub translate_x: f32,
    pub translate_y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation: f32, // radians
    pub skew_x: f32,
    pub skew_y: f32,
}

impl Transform {
    pub fn identity() -> Self {
        Self {
            translate_x: 0.0,
            translate_y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation: 0.0,
            skew_x: 0.0,
            skew_y: 0.0,
        }
    }

    pub fn translate(x: f32, y: f32) -> Self {
        Self {
            translate_x: x,
            translate_y: y,
            ..Self::identity()
        }
    }

    pub fn scale(sx: f32, sy: f32) -> Self {
        Self {
            scale_x: sx,
            scale_y: sy,
            ..Self::identity()
        }
    }

    pub fn rotate(radians: f32) -> Self {
        Self {
            rotation: radians,
            ..Self::identity()
        }
    }

    pub fn rotate_deg(degrees: f32) -> Self {
        Self::rotate(degrees.to_radians())
    }

    /// Combine two transforms
    pub fn then(self, other: Transform) -> Self {
        Self {
            translate_x: self.translate_x + other.translate_x,
            translate_y: self.translate_y + other.translate_y,
            scale_x: self.scale_x * other.scale_x,
            scale_y: self.scale_y * other.scale_y,
            rotation: self.rotation + other.rotation,
            skew_x: self.skew_x + other.skew_x,
            skew_y: self.skew_y + other.skew_y,
        }
    }

    /// Convert to 3x3 transformation matrix
    pub fn to_matrix(&self) -> [[f32; 3]; 3] {
        let cos_r = self.rotation.cos();
        let sin_r = self.rotation.sin();

        // Scale * Rotation * Translation
        [
            [self.scale_x * cos_r, -self.scale_y * sin_r, self.translate_x],
            [self.scale_x * sin_r, self.scale_y * cos_r, self.translate_y],
            [0.0, 0.0, 1.0],
        ]
    }
}

impl Animatable for Transform {
    fn interpolate(from: &Self, to: &Self, t: f32) -> Self {
        Self {
            translate_x: f32::interpolate(&from.translate_x, &to.translate_x, t),
            translate_y: f32::interpolate(&from.translate_y, &to.translate_y, t),
            scale_x: f32::interpolate(&from.scale_x, &to.scale_x, t),
            scale_y: f32::interpolate(&from.scale_y, &to.scale_y, t),
            rotation: f32::interpolate(&from.rotation, &to.rotation, t),
            skew_x: f32::interpolate(&from.skew_x, &to.skew_x, t),
            skew_y: f32::interpolate(&from.skew_y, &to.skew_y, t),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::color::{Color, Rgba};
    use crate::core::geometry::{Point, Size};
    use std::f32::consts::PI;
    use std::thread;
    use std::time::Duration;

    // ==================== Easing Function Tests ====================

    /// Helper to compare floats with tolerance
    fn approx_eq(a: f32, b: f32, tolerance: f32) -> bool {
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
        assert!(out_at_end > 1.0, "EaseOutBack should overshoot 1.0 near end");
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
            assert!(result >= 0.0 && result <= 1.1, "EaseOutBounce({}) = {} should be in [0, 1.1]", t, result);
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
        assert!(result.is_finite(), "Critically damped spring should produce finite values");
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
        let test_cases = vec![
            Easing::Linear,
            Easing::EaseIn,
            Easing::EaseOut,
        ];

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
        let cloned = easing.clone();
        let copied = easing;

        assert_eq!(easing, cloned);
        assert_eq!(easing, copied);
    }

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
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let mut anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_millis(10))
            .on_complete(move || {
                called_clone.store(true, Ordering::SeqCst);
            });

        anim.start();
        thread::sleep(Duration::from_millis(50));
        anim.update();

        assert!(called.load(Ordering::SeqCst), "on_complete should be called");
    }

    #[test]
    fn test_animation_with_easing() {
        let mut anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_millis(100))
            .easing(Easing::EaseIn);

        anim.start();
        thread::sleep(Duration::from_millis(50));

        let value = anim.value();
        // EaseIn should be slower at start, so value at 50% time should be < 50
        assert!(value < 50.0, "EaseIn should produce value < 50 at midpoint");
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
        assert!(approx_eq(matrix[0][0], 0.0, 0.001));  // cos
        assert!(approx_eq(matrix[0][1], -1.0, 0.001)); // -sin
        assert!(approx_eq(matrix[1][0], 1.0, 0.001));  // sin
        assert!(approx_eq(matrix[1][1], 0.0, 0.001));  // cos
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
        use std::sync::atomic::{AtomicU32, Ordering};
        use std::sync::Arc;

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
            Easing::Spring { stiffness: 100.0, damping: 10.0 },
            Easing::Custom(|t| t),
        ];

        for easing in easings {
            let mut anim = Animation::new(0.0_f32, 100.0_f32, Duration::from_millis(20))
                .easing(easing.clone());

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
}
