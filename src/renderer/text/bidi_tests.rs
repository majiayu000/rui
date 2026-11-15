//! Lines wrapped out of a paragraph keep the paragraph's base direction.

use super::*;

fn request(content: &str) -> TextRequest<'_> {
    TextRequest::new(content, 20.0, 400, None, 1.2)
}

/// `first` and `second` joined by a space, wrapped at the wider of the two.
fn wrap_two(cache: &mut TextMeasureCache, first: &str, second: &str) -> Vec<TextLine> {
    let mut width = |line| {
        let metrics = cache.measure_single_line(request(line));
        metrics.expect("measure").advance_width
    };
    let width = width(first).max(width(second)) + 1.0;
    let content = format!("{first} {second}");
    cache
        .wrap(request(&content), width, None)
        .expect("wrap")
        .lines
}

/// Left edge of the run holding byte `at` of the shaped line.
fn run_x(plan: &TextShapePlan, at: usize) -> f32 {
    let run = plan
        .runs()
        .iter()
        .find(|run| (run.byte_start..run.byte_end).contains(&at));
    run.expect("a run holds the byte").x_offset
}

#[test]
fn a_wrapped_line_starting_with_arabic_stays_left_to_right() {
    let mut cache = TextMeasureCache::new();
    if !cache.has_fonts() {
        return;
    }
    let lines = wrap_two(&mut cache, "layout glyph", "مرحبا بالعالم end");
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(&*lines[1].content, "مرحبا بالعالم end");
    assert!(
        lines
            .iter()
            .all(|line| line.direction == BaseDirection::LeftToRight)
    );

    // At the paragraph's level the Arabic reads first, on the left, and the
    // Latin word after it; at a level of the line's own it would be reversed.
    let line = &lines[1];
    let latin = line.content.find("end").expect("latin word");
    let plan = shape(
        &mut cache,
        request(&line.content).with_direction(line.direction),
    );
    assert!(run_x(&plan, 0) < run_x(&plan, latin), "{:?}", plan.runs());
    let own = shape(&mut cache, request(&line.content));
    assert!(run_x(&own, 0) > run_x(&own, latin), "{:?}", own.runs());
}

#[test]
fn a_wrapped_line_starting_with_latin_stays_right_to_left() {
    let mut cache = TextMeasureCache::new();
    if !cache.has_fonts() {
        return;
    }
    let lines = wrap_two(&mut cache, "مرحبا بالعالم", "layout النص");
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(&*lines[1].content, "layout النص");
    assert!(
        lines
            .iter()
            .all(|line| line.direction == BaseDirection::RightToLeft)
    );

    let line = &lines[1];
    let arabic = line.content.find('ا').expect("arabic word");
    let plan = shape(
        &mut cache,
        request(&line.content).with_direction(line.direction),
    );
    assert!(run_x(&plan, 0) > run_x(&plan, arabic), "{:?}", plan.runs());
}

#[test]
fn shapes_and_measures_are_kept_per_base_direction() {
    let mut cache = TextMeasureCache::new();
    if !cache.has_fonts() {
        return;
    }
    let content = "مرحبا end";
    let ltr = request(content).with_direction(BaseDirection::LeftToRight);
    let rtl = request(content).with_direction(BaseDirection::RightToLeft);
    cache.measure_single_line(ltr).expect("measure");
    cache.measure_single_line(rtl).expect("measure");
    assert_eq!(cache.stats().measure_misses, 2);
    cache.measure_single_line(rtl).expect("measure");
    assert_eq!(cache.stats().measure_hits, 1);
}

fn shape(cache: &mut TextMeasureCache, request: TextRequest<'_>) -> TextShapePlan {
    cache.shape_single_line(request).expect("shape")
}
