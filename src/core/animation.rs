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
