use std::cell::Cell;

use super::{
    OUTPUT_SIZE_BYTES_MAX, Output, Render, RenderError, TrustedHtml, each, fragment, lazy, render,
    render_with_limit,
};

thread_local! {
    static RESERVATIONS_BEFORE_FAILURE: Cell<Option<u32>> = const { Cell::new(None) };
}

pub(super) fn append_reservation_size(size_bytes: usize) -> usize {
    RESERVATIONS_BEFORE_FAILURE.with(|remaining| match remaining.get() {
        Some(0) => usize::MAX,
        Some(count) => {
            remaining.set(Some(count - 1));
            size_bytes
        }
        None => size_bytes,
    })
}

struct ReservationFailure {
    previous: Option<u32>,
}

impl ReservationFailure {
    fn after(success_count: u32) -> Self {
        Self {
            previous: RESERVATIONS_BEFORE_FAILURE.replace(Some(success_count)),
        }
    }

    fn remaining(&self) -> u32 {
        RESERVATIONS_BEFORE_FAILURE
            .get()
            .expect("reservation fault injection must be enabled")
    }
}

impl Drop for ReservationFailure {
    fn drop(&mut self) {
        RESERVATIONS_BEFORE_FAILURE.set(self.previous);
    }
}

#[test]
fn reservation_failures_propagate_at_every_growth_point() {
    const RESERVATION_COUNT_MAX: u32 = 32;
    let text = "<&>\"é".repeat(200);
    let trusted = TrustedHtml(&text);
    let owned = render(&text).unwrap();
    let attribute = fragment(|output| output.attribute("data-on:submit", &text));
    let padding = "x".repeat(100);
    let numeric = fragment(|output| {
        output.trusted(TrustedHtml("<div"))?;
        output.attribute("padding", &padding)?;
        output.attribute("count", &u128::MAX)?;
        output.trusted(TrustedHtml(">"))?;
        f64::MAX.render(output)?;
        output.trusted(TrustedHtml("</div>"))
    });
    let owned_reference = &owned;
    let items = [text.as_str(); 3];
    let nested = html! {
        <main>
            {owned_reference}<ul>{each(&items, |item| html! { <li title={item}>{item}</li> })}</ul>
        </main>
    };
    let optional = Some(&nested);
    let collected = vec![&nested, &nested];
    let deferred = lazy(|| &nested);
    let script = html! { <script>{TrustedHtml(&text)}</script> };
    let style = html! { <style>{TrustedHtml(&text)}</style> };
    let values: [(&str, &dyn Render); 12] = [
        ("text", &text),
        ("trusted", &trusted),
        ("owned", &owned),
        ("attribute", &attribute),
        ("numeric", &numeric),
        ("nested", &nested),
        ("optional", &optional),
        ("collected", &collected),
        ("slice", &collected.as_slice()),
        ("lazy", &deferred),
        ("script", &script),
        ("style", &style),
    ];
    for (name, value) in values {
        let expected = render(value).unwrap();
        let reservation_count = {
            let failure = ReservationFailure::after(RESERVATION_COUNT_MAX);
            assert_eq!(render(value).unwrap(), expected, "{name}");
            RESERVATION_COUNT_MAX - failure.remaining()
        };
        assert!(reservation_count > 0, "{name}");
        for success_count in 0..reservation_count {
            let completed = Cell::new(false);
            let checked = fragment(|output| {
                value.render(output)?;
                completed.set(true);
                Ok(())
            });
            {
                let failure = ReservationFailure::after(success_count);
                assert_eq!(
                    render(&checked),
                    Err(RenderError::Allocation),
                    "{name} after {success_count}"
                );
                assert!(!completed.get(), "{name}");
                assert_eq!(failure.remaining(), 0);
            }
            {
                let _failure = ReservationFailure::after(success_count);
                let mut output = Output::new(OUTPUT_SIZE_BYTES_MAX);
                assert_eq!(
                    value.render(&mut output),
                    Err(RenderError::Allocation),
                    "{name} after {success_count}"
                );
                assert!(expected.as_str().starts_with(&output.value), "{name}");
                assert!(output.value.len() <= OUTPUT_SIZE_BYTES_MAX as usize);
            }
        }
        let failure = ReservationFailure::after(reservation_count);
        assert_eq!(render(value).unwrap(), expected, "{name}");
        assert_eq!(failure.remaining(), 0);
    }
}

#[test]
fn failed_growth_preserves_existing_bytes_and_capacity() {
    let mut output = Output::new(256);
    output.text("prefix").unwrap();
    let before = output.value.clone();
    let capacity_bytes = output.value.capacity();
    let text = "x".repeat(capacity_bytes + 1);
    let _failure = ReservationFailure::after(0);
    assert_eq!(
        output.trusted(TrustedHtml(&text)),
        Err(RenderError::Allocation)
    );
    assert_eq!(output.value, before);
    assert_eq!(output.value.capacity(), capacity_bytes);
}

#[test]
fn omitted_content_and_limits_do_not_attempt_reservation() {
    let failure = ReservationFailure::after(0);
    assert_eq!(render("").unwrap().as_str(), "");
    let omitted = fragment(|output| {
        output.attribute("disabled", &false)?;
        output.attribute("title", &None::<&str>)
    });
    assert_eq!(render(&omitted).unwrap().as_str(), "");
    assert_eq!(render_with_limit("&", 4), Err(RenderError::OutputLimit));
    assert_eq!(failure.remaining(), 0);
}

#[test]
fn reusable_output_retains_capacity_and_clears_failed_documents() {
    let mut output = Output::new(256);
    let page = html! { <p>{"<&>"}</p> };
    let expected = render(&page).unwrap();
    assert_eq!(output.render(&page).unwrap(), expected.as_str());
    let capacity_size_bytes = output.value.capacity();
    {
        let failure = ReservationFailure::after(0);
        for _ in 0..8 {
            assert_eq!(output.render(&page).unwrap(), expected.as_str());
            assert_eq!(output.value.capacity(), capacity_size_bytes);
        }
        let larger = "x".repeat(capacity_size_bytes + 1);
        let growth = fragment(|output| {
            output.text("prefix")?;
            output.text(&larger)
        });
        assert_eq!(output.render(&growth), Err(RenderError::Allocation));
        assert!(output.value.is_empty());
        assert_eq!(output.value.capacity(), capacity_size_bytes);
        assert_eq!(failure.remaining(), 0);
    }
    assert_eq!(output.render("fresh").unwrap(), "fresh");
    assert_eq!(output.render("good&bad\0"), Err(RenderError::NullCharacter));
    assert!(output.value.is_empty());
    let invalid_raw = html! { <script>"</script>"</script> };
    assert_eq!(
        output.render(&invalid_raw),
        Err(RenderError::InvalidRawText)
    );
    assert!(output.value.is_empty());
    assert_eq!(
        output.render(&"x".repeat(257)),
        Err(RenderError::OutputLimit)
    );
    assert!(output.value.is_empty());
    assert_eq!(output.render("").unwrap(), "");
    assert_eq!(output.render("next").unwrap(), "next");
}

#[test]
#[ignore = "manual buffer reuse measurement; run in release mode with --nocapture"]
fn measure_buffer_reuse() {
    use std::{hint::black_box, time::Instant};

    const SAMPLE_COUNT: usize = 16;
    const RENDER_COUNT: u32 = 2_000;
    for size_bytes in [64, 8_192] {
        let text = "x".repeat(size_bytes);
        let page = html! { <article>{text}</article> };
        let expected = render(&page).unwrap();
        let mut output = Output::new(OUTPUT_SIZE_BYTES_MAX);
        assert_eq!(output.render(&page).unwrap(), expected.as_str());
        let mut fresh_ns = [0; SAMPLE_COUNT];
        let mut reused_ns = [0; SAMPLE_COUNT];
        for sample_index in 0..SAMPLE_COUNT {
            for reuse in [sample_index % 2 == 0, sample_index % 2 != 0] {
                let start = Instant::now();
                for _ in 0..RENDER_COUNT {
                    if reuse {
                        black_box(output.render(black_box(&page)).unwrap());
                    } else {
                        black_box(render(black_box(&page)).unwrap());
                    }
                }
                let elapsed_ns = start.elapsed().as_nanos() / u128::from(RENDER_COUNT);
                if reuse {
                    reused_ns[sample_index] = elapsed_ns;
                } else {
                    fresh_ns[sample_index] = elapsed_ns;
                }
            }
        }
        fresh_ns.sort();
        reused_ns.sort();
        println!(
            "{size_bytes} bytes: fresh ns median={} range={}..{}; reused ns median={} range={}..{}",
            fresh_ns[SAMPLE_COUNT / 2],
            fresh_ns[0],
            fresh_ns[SAMPLE_COUNT - 1],
            reused_ns[SAMPLE_COUNT / 2],
            reused_ns[0],
            reused_ns[SAMPLE_COUNT - 1]
        );
        {
            let failure = ReservationFailure::after(32);
            render(&page).unwrap();
            println!("fresh reservation attempts={}", 32 - failure.remaining());
        }
        let failure = ReservationFailure::after(0);
        assert_eq!(output.render(&page).unwrap(), expected.as_str());
        assert_eq!(failure.remaining(), 0);
        println!("warm reused reservation attempts=0");
    }
}
