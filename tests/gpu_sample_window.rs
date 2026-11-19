//! The RUI/GPUI comparison keeps the same GPU samples on both sides: frames 10 to 69
//! of every window, as many as it drew. This checks that choice without a GPU.
#![cfg(target_os = "macos")]

// The benches use the rest of this file; this test only its sample selection.
#[allow(dead_code)]
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
    // Criterion opens a new window per call, each starting its warm-up again: one
    // closed during its warm-up gives nothing, a short one its frames from 10 on.
    let mut tagged = window(0, 3);
    tagged.extend(window(1, 200));
    tagged.extend(window(2, 35));
    // Completion order is not frame order.
    tagged.reverse();
    let (samples, windows) = steady_samples(&tagged, 10, 60).expect("two windows");
    assert_eq!(windows, 2);
    assert_eq!(samples.len(), 60 + 25);
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

    assert!(
        steady_samples(&window(0, 10), 10, 60).is_err(),
        "nothing after warm-up"
    );
}

#[test]
fn redraws_give_each_sampled_frame_their_mean() {
    use gpu_time::{redraw_tag, steady_redraws};
    // Frame f takes 1 ms, and its three redraws f, f + 1 and f + 2 ms.
    let mut tagged = Vec::new();
    for window_ in 0..2 {
        for frame in 0..70 {
            let tag_ = tag(window_, frame);
            tagged.push((tag_, 1.0));
            for redraw in 0..3 {
                tagged.push((redraw_tag(tag_), (frame + redraw) as f64));
            }
        }
    }
    let means = steady_redraws(&tagged, 10, 60).expect("redrawn frames");
    assert_eq!(means.len(), 120);
    assert_eq!(means[0], 11.0, "window 0 frame 10");
    assert_eq!(means[60], 11.0, "window 1 frame 10");
    // The redraws are not frames of their own.
    let (frames, _) = steady_samples(&tagged, 10, 60).expect("frames");
    assert!(frames.iter().all(|&ms| ms == 1.0));

    tagged.retain(|&(tag_, _)| tag_ != redraw_tag(tag(1, 40)));
    assert!(
        steady_redraws(&tagged, 10, 60).is_err(),
        "a frame without redraws"
    );
}
