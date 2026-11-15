//! The RUI/GPUI comparison keeps the same GPU samples on both sides: frames 10 to 69
//! of every window that reached them. This checks that choice without a GPU.
#![cfg(target_os = "macos")]

#[path = "../benchmarks/gpui_compare/gpu_time.rs"]
mod gpu_time;

use gpu_time::{steady_samples, tag};

/// One command buffer per frame `0..frames` of `window`, each taking `frame` ms.
fn window(window: usize, frames: usize) -> Vec<(usize, f64)> {
    (0..frames)
        .map(|frame| (tag(window, frame), frame as f64))
        .collect()
}

#[test]
fn warm_up_frames_are_left_out() {
    let (samples, windows) = steady_samples(&window(0, 70), 10, 60).expect("one full window");
    assert_eq!(windows, 1);
    assert_eq!(samples.len(), 60);
    assert_eq!(samples.first(), Some(&10.0), "frames 0..10 are warm-up");
    assert_eq!(samples.last(), Some(&69.0));
}

#[test]
fn frames_after_the_sample_are_left_out() {
    let (samples, _) = steady_samples(&window(0, 500), 10, 60).expect("one long window");
    assert_eq!(samples.len(), 60);
    assert_eq!(samples.last(), Some(&69.0));
}

#[test]
fn every_window_contributes_its_own_steady_frames() {
    // Criterion opens a new window per call: each starts its warm-up again, and a
    // short first call does not reach the sample.
    let mut tagged = window(0, 3);
    tagged.extend(window(1, 200));
    tagged.extend(window(2, 70));
    // Completion order is not frame order.
    tagged.reverse();
    let (samples, windows) = steady_samples(&tagged, 10, 60).expect("two full windows");
    assert_eq!(windows, 2);
    assert_eq!(samples.len(), 120);
    assert!(samples.iter().all(|&ms| (10.0..70.0).contains(&ms)));
}

#[test]
fn a_frame_with_two_buffers_or_none_is_an_error() {
    let mut twice = window(0, 70);
    twice.push((tag(0, 20), 1.0));
    assert!(steady_samples(&twice, 10, 60).is_err());

    let mut gap = window(0, 70);
    gap.retain(|&(tag_, _)| tag_ != tag(0, 30));
    assert!(steady_samples(&gap, 10, 60).is_err());

    assert!(steady_samples(&window(0, 40), 10, 60).is_err(), "no window reached frame 69");
}
