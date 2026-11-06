//! Frames drawn through a `RetainedTarget`, which redraws only the damage, must
//! be pixel for pixel the frames a full redraw draws.

#[path = "support/random_tree.rs"]
mod random_tree;

use random_tree::{Incremental, Node, Rng};
use rui::renderer::metal::{MetalRenderer, RetainedTarget};
use rui::renderer::text::TextMeasureCache;
use rui::renderer::{Reach, RendererError};
use rui::{Bounds, Color, Div, Image, ImageFit, Size, div, text};

const SCALE: f32 = 2.0;

fn renderer() -> Option<MetalRenderer> {
    match MetalRenderer::new() {
        Ok(renderer) => Some(renderer),
        Err(RendererError::BackendUnavailable(_)) => None,
        Err(err) => panic!("metal renderer should initialize: {err}"),
    }
}

/// Draw the incremental runner's scene both ways and compare. Returns the device
/// pixels the retained frame drew again.
fn assert_same_pixels(
    renderer: &mut MetalRenderer,
    target: &mut RetainedTarget,
    incremental: &Incremental,
    viewport: Size,
    context: &str,
) -> u64 {
    let retained = match renderer.capture_retained_pixels_at_scale(
        &incremental.scene,
        target,
        viewport,
        SCALE,
    ) {
        Ok(pixels) => pixels,
        Err(err) => panic!("{context}: retained capture failed: {err}"),
    };
    let redrawn = renderer.last_frame_stats().redrawn_pixels;
    let full = match renderer.capture_frame_pixels_at_scale(&incremental.scene, viewport, SCALE) {
        Ok(pixels) => pixels,
        Err(err) => panic!("{context}: full capture failed: {err}"),
    };
    assert_eq!(
        retained.len(),
        full.len(),
        "{context}: capture sizes differ"
    );
    if let Some(index) = (0..full.len()).find(|&index| retained[index] != full[index]) {
        let width = (viewport.width * SCALE).round() as usize;
        let pixel = index / 4;
        panic!(
            "{context}: pixel ({}, {}) is {:?} redrawn in the damage, {:?} in a full redraw",
            pixel % width,
            pixel / width,
            &retained[pixel * 4..pixel * 4 + 4],
            &full[pixel * 4..pixel * 4 + 4]
        );
    }
    redrawn
}

#[test]
fn damaged_frames_match_full_redraws_over_random_edits() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut measurer = TextMeasureCache::new();
    let mut partial = 0;
    for seed in 1..=12u64 {
        let mut rng = Rng(seed.wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1);
        let mut model = Node::random(&mut rng, 4);
        let mut incremental = Incremental::new();
        let mut target = RetainedTarget::new();
        incremental.rebuild(model.build());
        let mut viewport = Size::new(320.0, 240.0);
        for frame in 0..16 {
            match rng.below(6) {
                0 => {}
                1 => {
                    viewport =
                        Size::new(160.0 + rng.below(240) as f32, 120.0 + rng.below(200) as f32);
                    incremental.rebuild(model.build());
                }
                _ => {
                    let at = rng.below(model.count() as u64) as usize;
                    model.nth(at).edit(&mut rng);
                    incremental.rebuild(model.build());
                }
            }
            incremental.frame(&mut measurer, viewport);
            let total =
                (viewport.width * SCALE).round() as u64 * (viewport.height * SCALE).round() as u64;
            let redrawn = assert_same_pixels(
                &mut renderer,
                &mut target,
                &incremental,
                viewport,
                &format!("seed {seed}, frame {frame}"),
            );
            if redrawn < total {
                partial += 1;
            }
        }
    }
    assert!(partial > 0, "no frame redrew less than the whole target");
}

fn list(edited: usize, round: usize) -> Div {
    div().w(320.0).flex_col().children((0..30).map(move |row| {
        let label = if row == edited {
            format!("edited {round}")
        } else {
            format!("row {row}")
        };
        div()
            .h(20.0)
            .bg(Color::hex(0x223344 + (row as u32 % 3) * 0x111111))
            .child(text(label))
    }))
}

#[test]
fn one_changed_row_redraws_about_one_row() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut measurer = TextMeasureCache::new();
    let viewport = Size::new(320.0, 600.0);
    let mut incremental = Incremental::new();
    let mut target = RetainedTarget::new();
    let total = (320.0 * SCALE * 600.0 * SCALE) as u64;
    for round in 0..6 {
        incremental.rebuild(list(7, round));
        incremental.frame(&mut measurer, viewport);
        let redrawn = assert_same_pixels(
            &mut renderer,
            &mut target,
            &incremental,
            viewport,
            &format!("round {round}"),
        );
        if round == 0 {
            assert_eq!(redrawn, total, "the first frame draws everything");
        } else {
            // A 20 px row plus what its text may reach (twice the font size above
            // and below, see `Primitive::reach`): 78 of 600 px tall.
            assert!(
                redrawn * 5 < total,
                "round {round} redrew {redrawn} of {total} pixels"
            );
            assert!(matches!(
                incremental
                    .scene
                    .damage(Bounds::from_xywh(0.0, 0.0, 320.0, 600.0)),
                Reach::Within(_)
            ));
        }
    }
}

#[test]
fn an_unchanged_frame_redraws_nothing_and_a_resize_everything() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut measurer = TextMeasureCache::new();
    let mut incremental = Incremental::new();
    let mut target = RetainedTarget::new();
    incremental.rebuild(list(3, 0));
    for (round, viewport) in [(320.0, 400.0), (320.0, 400.0), (300.0, 400.0)]
        .into_iter()
        .enumerate()
    {
        let viewport = Size::new(viewport.0, viewport.1);
        incremental.frame(&mut measurer, viewport);
        let redrawn = assert_same_pixels(
            &mut renderer,
            &mut target,
            &incremental,
            viewport,
            &format!("round {round}"),
        );
        let total = (viewport.width * SCALE * viewport.height * SCALE) as u64;
        match round {
            1 => assert_eq!(redrawn, 0, "nothing changed"),
            _ => assert_eq!(redrawn, total, "a new size draws everything"),
        }
    }
}

#[test]
fn a_target_that_missed_a_frame_redraws_everything() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut measurer = TextMeasureCache::new();
    let viewport = Size::new(320.0, 400.0);
    let mut incremental = Incremental::new();
    let mut target = RetainedTarget::new();
    incremental.rebuild(list(3, 0));
    incremental.frame(&mut measurer, viewport);
    assert_same_pixels(&mut renderer, &mut target, &incremental, viewport, "first");
    // Painted but never presented: the target still shows the first frame.
    incremental.rebuild(list(3, 1));
    incremental.frame(&mut measurer, viewport);
    incremental.rebuild(list(3, 2));
    incremental.frame(&mut measurer, viewport);
    let redrawn = assert_same_pixels(&mut renderer, &mut target, &incremental, viewport, "third");
    assert_eq!(redrawn, (320.0 * SCALE * 400.0 * SCALE) as u64);
}

#[test]
fn rows_moved_by_a_growing_row_are_redrawn_at_both_places() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut measurer = TextMeasureCache::new();
    let viewport = Size::new(320.0, 600.0);
    // No background on the column, so only the moved rows cover where they were.
    let rows = |grown: f32| {
        div().w(320.0).flex_col().children((0..20).map(move |row| {
            let height = if row == 4 { 20.0 + grown } else { 20.0 };
            div()
                .h(height)
                .bg(Color::hex(0x223344 + (row as u32 % 3) * 0x111111))
                .child(text(format!("row {row}")))
        }))
    };
    let mut incremental = Incremental::new();
    let mut target = RetainedTarget::new();
    for (round, grown) in [0.0, 6.0, 2.0, 30.0, 0.0].into_iter().enumerate() {
        incremental.rebuild(rows(grown));
        incremental.frame(&mut measurer, viewport);
        assert_same_pixels(
            &mut renderer,
            &mut target,
            &incremental,
            viewport,
            &format!("round {round}"),
        );
        if round > 0 {
            assert!(
                incremental.scene.replayed_len() > 0,
                "round {round} replayed nothing"
            );
        }
    }
}

#[test]
fn rows_scrolled_out_and_back_redraw_correctly() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut measurer = TextMeasureCache::new();
    let viewport = Size::new(320.0, 200.0);
    let column = |spacer: f32| {
        div()
            .w(320.0)
            .flex_col()
            .child(div().h(spacer))
            .children((0..200).map(|row| {
                div()
                    .h(20.0)
                    .bg(Color::hex(0x223344 + (row as u32 % 3) * 0x111111))
                    .child(text(format!("row {row}")))
            }))
    };
    let mut incremental = Incremental::new();
    let mut target = RetainedTarget::new();
    for (round, spacer) in [0.0, 40.0, 300.0, 0.0, 1_000.0, 10.0]
        .into_iter()
        .enumerate()
    {
        incremental.rebuild(column(spacer));
        incremental.frame(&mut measurer, viewport);
        assert_same_pixels(
            &mut renderer,
            &mut target,
            &incremental,
            viewport,
            &format!("round {round}"),
        );
    }
}

/// A `width` by `height` image with a different color in every pixel, so a
/// misplaced or stale sample shows.
fn gradient(width: u32, height: u32) -> Vec<u8> {
    (0..width * height)
        .flat_map(|index| {
            let (x, y) = (index % width, index / width);
            [
                (x * 255 / width) as u8,
                (y * 255 / height) as u8,
                0x80,
                0xff,
            ]
        })
        .collect()
}

const FITS: [ImageFit; 5] = [
    ImageFit::Fill,
    ImageFit::Contain,
    ImageFit::Cover,
    ImageFit::None,
    ImageFit::ScaleDown,
];

/// Rows of images in every fit mode, wide and tall, from data and from a
/// file, under a colored strip and above a list with one edited row.
fn gallery(file: &str, spacer: f32, shift: usize, edited: usize, strip: u32) -> Div {
    let images = (0..FITS.len()).map(move |index| {
        let fit = FITS[(index + shift) % FITS.len()];
        let row = div().flex_row().gap(4.0);
        // Wide, tall, and larger than the box, so cover and none overflow it.
        let row = [(12, 5), (3, 9), (90, 70)]
            .into_iter()
            .fold(row, |row, (width, height)| {
                row.child(
                    Image::from_data(gradient(width, height), width, height)
                        .fit(fit)
                        .size(Size::new(44.0, 30.0)),
                )
            });
        row.child(
            Image::from_file(file)
                .fit(fit)
                .rounded(6.0)
                .size(Size::new(30.0, 44.0)),
        )
    });
    // Right over the images: its antialiased edge reaches a pixel into the
    // first row, which must be drawn again with it.
    let strip = div().h(6.0).bg(Color::hex(0x406080 + strip * 0x0a0a0a));
    div()
        .w(320.0)
        .flex_col()
        .child(div().h(spacer))
        .child(strip)
        .children(images)
        .child(list(edited, edited))
}

#[test]
fn images_in_every_fit_mode_redraw_only_their_damage() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let dir = std::env::temp_dir().join(format!("rui-damage-images-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create image dir");
    let file = dir.join("tall.png");
    image::RgbaImage::from_raw(6, 16, gradient(6, 16))
        .expect("image size matches its pixels")
        .save(&file)
        .expect("write image file");
    let file = file.to_str().expect("utf-8 temp path").to_string();

    let mut measurer = TextMeasureCache::new();
    let viewport = Size::new(320.0, 600.0);
    let total = (320.0 * SCALE * 600.0 * SCALE) as u64;
    let mut incremental = Incremental::new();
    let mut target = RetainedTarget::new();
    // The strip recolored, rows edited, the images moved down and back, and
    // their fits changed; the last element is the most of the target the frame
    // may draw again.
    let rounds = [
        (0.0, 0, 10, 0, 1.0),
        (0.0, 0, 10, 1, 0.05),
        (0.0, 0, 11, 1, 0.7),
        (7.5, 0, 11, 1, 1.0),
        (7.5, 2, 12, 2, 1.0),
        (0.0, 4, 13, 2, 1.0),
        (0.0, 1, 13, 2, 0.5),
        (0.0, 1, 13, 2, 0.0),
    ];
    for (round, (spacer, shift, edited, strip, most)) in rounds.into_iter().enumerate() {
        incremental.rebuild(gallery(&file, spacer, shift, edited, strip));
        incremental.frame(&mut measurer, viewport);
        let damage = incremental
            .scene
            .damage(Bounds::from_xywh(0.0, 0.0, 320.0, 600.0));
        let redrawn = assert_same_pixels(
            &mut renderer,
            &mut target,
            &incremental,
            viewport,
            &format!("round {round}"),
        );
        if round > 0 {
            assert!(
                !matches!(damage, Reach::Unbounded),
                "round {round}: images made the damage unbounded"
            );
        }
        eprintln!(
            "round {round}: redrew {:.3} of the target",
            redrawn as f64 / total as f64
        );
        assert!(
            redrawn as f64 <= total as f64 * most,
            "round {round} redrew {redrawn} of {total} pixels"
        );
    }
    std::fs::remove_dir_all(&dir).expect("remove image dir");
}

#[test]
fn an_edit_away_from_the_images_leaves_them_out_of_the_damage() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut measurer = TextMeasureCache::new();
    let viewport = Size::new(320.0, 600.0);
    let images = |round: usize| {
        div()
            .w(320.0)
            .flex_col()
            .child(list(29, round))
            .children(FITS.map(|fit| {
                div().flex_row().child(
                    Image::from_data(gradient(5, 3), 5, 3)
                        .fit(fit)
                        .size(Size::new(60.0, 20.0)),
                )
            }))
    };
    let mut incremental = Incremental::new();
    let mut target = RetainedTarget::new();
    for round in 0..3 {
        incremental.rebuild(images(round));
        incremental.frame(&mut measurer, viewport);
        let damage = incremental
            .scene
            .damage(Bounds::from_xywh(0.0, 0.0, 320.0, 600.0));
        assert_same_pixels(
            &mut renderer,
            &mut target,
            &incremental,
            viewport,
            &format!("round {round}"),
        );
        if round > 0 {
            // The edited row is the 30th, 580 px down; the images sit at 600 and
            // below, past the window, and are not drawn.
            let Reach::Within(damage) = damage else {
                panic!("round {round}: damage {damage:?}");
            };
            assert!(
                damage.y() > 500.0,
                "round {round}: damage {damage:?} reaches the top rows"
            );
        }
    }
}
