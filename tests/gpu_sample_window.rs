//! The RUI/GPUI comparison keeps the same GPU samples on both sides: a complete
//! interval from one window. This checks that choice without a GPU.
#![cfg(target_os = "macos")]

#[path = "../benchmarks/gpui_compare/gpu_time.rs"]
mod gpu_time;

use gpu_time::{OUTSIDE_SAMPLE, steady_samples};

/// One command buffer per frame `0..frames`, each taking `frame` ms.
fn window(frames: usize) -> Vec<(usize, f64)> {
    (0..frames).map(|frame| (frame, frame as f64)).collect()
}

#[test]
fn warm_up_frames_are_left_out() {
    let samples = steady_samples(&window(70), 10, 60).expect("one full window");
    assert_eq!(samples.len(), 60);
    assert_eq!(samples.first(), Some(&10.0), "frames 0..10 are warm-up");
    assert_eq!(samples.last(), Some(&69.0));
}

#[test]
fn frames_after_the_sample_are_left_out() {
    let samples = steady_samples(&window(500), 10, 60).expect("one long window");
    assert_eq!(samples.len(), 60);
    assert_eq!(samples.last(), Some(&69.0));
}

#[test]
fn criterion_and_setup_buffers_do_not_enter_the_sample() {
    let mut tagged = window(70);
    tagged.extend([(OUTSIDE_SAMPLE, 20_000.0); 3]);
    // Completion order is not frame order.
    tagged.reverse();
    let samples = steady_samples(&tagged, 10, 60).expect("one full window");
    assert_eq!(samples, (10..70).map(f64::from).collect::<Vec<_>>());
}

#[test]
fn a_frame_with_two_buffers_or_none_is_an_error() {
    let mut twice = window(70);
    twice.push((20, 1.0));
    assert!(steady_samples(&twice, 10, 60).is_err());

    let mut gap = window(70);
    gap.retain(|&(tag, _)| tag != 30);
    assert!(steady_samples(&gap, 10, 60).is_err());

    assert!(
        steady_samples(&window(10), 10, 60).is_err(),
        "nothing after warm-up"
    );
    assert_eq!(
        steady_samples(&window(69), 10, 60).unwrap_err(),
        "frame 69 has no command buffer",
        "a window ending during the sample is incomplete",
    );
}

#[test]
fn redraws_give_each_sampled_frame_their_mean() {
    use gpu_time::{redraw_tag, steady_redraws};
    // Frame f takes 1 ms, and its three redraws f, f + 1 and f + 2 ms.
    let mut tagged = Vec::new();
    for frame in 0..70 {
        tagged.push((frame, 1.0));
        for redraw in 0..3 {
            tagged.push((redraw_tag(frame), (frame + redraw) as f64));
        }
    }
    let means = steady_redraws(&tagged, 10, 60, 3).expect("redrawn frames");
    assert_eq!(means.len(), 60);
    assert_eq!(means[0], 11.0, "frame 10");
    assert_eq!(means[59], 70.0, "frame 69");
    // The redraws are not frames of their own.
    let frames = steady_samples(&tagged, 10, 60).expect("frames");
    assert!(frames.iter().all(|&ms| ms == 1.0));

    let missing = tagged
        .iter()
        .position(|&(tag, _)| tag == redraw_tag(40))
        .expect("frame 40 has redraws");
    tagged.remove(missing);
    assert_eq!(
        steady_redraws(&tagged, 10, 60, 3).unwrap_err(),
        "frame 40 has 2 redraws; expected 3",
    );
}
