use rstml_ssr::{
    Markup, Output, RenderError, RenderResult, TrustedHtml, html, render_scope,
    render_scope_with_limit,
};
use std::cell::Cell;

fn page(
    output: &mut Output,
    title: &str,
    body: impl FnOnce(&mut Output) -> RenderResult,
) -> RenderResult {
    let title = format!("{title} - Research");
    html! { output,
        <!DOCTYPE html>
        <html>
            <head>
                <title>{title}</title>
            </head>
            <body>{|output| body(output)}</body>
        </html>
    }
}

fn structured_data(output: &mut Output, name: &str) -> RenderResult {
    let json = serde_json::json!({"name": name})
        .to_string()
        .replace('<', "\\u003c");
    html! { output, <script type="application/ld+json">{TrustedHtml(&json)}</script> }
}

fn form(output: &mut Output, email: &str) -> RenderResult {
    let action = String::from("@post('/contact')");
    html! { output,
        <form data-on:submit=action>
            <input type="email" name="email" value=email required />
            <button type="submit">"Send"</button>
        </form>
    }
}

fn document(name: &str) -> RenderResult<Markup> {
    let addresses = [String::from("a<&>@example.test")];
    render_scope(|output| {
        let identity = std::ptr::from_ref(output);
        page(output, name, |output| {
            assert_eq!(identity, std::ptr::from_ref(output));
            structured_data(output, name)?;
            form(output, &addresses[0])
        })
    })
}

#[test]
fn components_share_one_scope_and_borrow_locals_without_returned_lifetimes() {
    let markup = document("</script><script>bad()</script>").unwrap();
    assert!(markup.as_str().starts_with("<!DOCTYPE html><html>"));
    assert!(
        markup
            .as_str()
            .contains("&lt;/script&gt;&lt;script&gt;bad()&lt;/script&gt; - Research")
    );
    assert_eq!(markup.as_str().matches("<script").count(), 1);
    assert_eq!(markup.as_str().matches("</script>").count(), 1);
    assert!(
        markup
            .as_str()
            .contains("value=\"a&lt;&amp;&gt;@example.test\" required=\"\"")
    );
}

#[test]
fn eager_html_owns_bytes_and_borrows_indexed_fields_without_moving_them() {
    struct Item {
        name: String,
    }
    let markup = {
        let items = [Item {
            name: String::from("<name>"),
        }];
        let calls = Cell::new(0);
        let markup = html! {
            <p title={items[0]
                .name}>
                {items[0].name}
                {{
                    calls.set(calls.get() + 1);
                    String::from("!")
                }}
            </p>
        }
        .unwrap();
        assert_eq!(calls.get(), 1);
        assert_eq!(items[0].name, "<name>");
        markup
    };
    assert_eq!(
        markup.as_str(),
        "<p title=\"&lt;name&gt;\">&lt;name&gt;!</p>"
    );
}

#[test]
fn components_can_consume_owned_values_once() {
    let value = String::from("owned");
    let markup = render_scope(move |output| {
        html! { output,
            <section>
                {move |output| {
                    let consumed = value.into_bytes();
                    output.text(std::str::from_utf8(&consumed).unwrap())
                }}
            </section>
        }
    })
    .unwrap();
    assert_eq!(markup.as_str(), "<section>owned</section>");
}

#[test]
fn one_budget_covers_parent_and_children_at_every_byte_boundary() {
    let full = document("é<&>").unwrap();
    for limit_bytes in 0..=full.as_str().len() as u32 + 1 {
        let result = render_scope_with_limit(
            |output| {
                page(output, "é<&>", |output| {
                    structured_data(output, "é<&>")?;
                    form(output, "a<&>@example.test")
                })
            },
            limit_bytes,
        );
        if limit_bytes < full.as_str().len() as u32 {
            assert_eq!(result, Err(RenderError::OutputLimit));
        } else {
            assert_eq!(result.unwrap().as_str(), full.as_str());
        }
    }
}

#[test]
fn scope_failures_discard_partial_documents_and_stop_later_expressions() {
    let mut output = Output::new(128);
    for error in [
        RenderError::OutputLimit,
        RenderError::Allocation,
        RenderError::InvalidAttributeName,
        RenderError::ItemLimit,
        RenderError::NullCharacter,
        RenderError::InvalidRawText,
    ] {
        let calls = Cell::new(0);
        assert_eq!(
            output.scope(|output| {
                html!(output,
                    <div>
                        { |output| { output.text("partial")?; Err(error) } }
                        { { calls.set(calls.get() + 1); "unreachable" } }
                    </div>
                )
            }),
            Err(error)
        );
        assert_eq!(calls.get(), 0);
        assert_eq!(output.scope(|_| Ok(())).unwrap(), "");
        assert_eq!(
            output.scope(|output| html!(output, <p>"next"</p>)).unwrap(),
            "<p>next</p>"
        );
    }
}

#[test]
fn list_components_validate_count_and_stop_at_the_first_write_error() {
    let calls = Cell::new(0_u32);
    let oversized = vec![(); rstml_ssr::ITEM_COUNT_MAX as usize + 1];
    assert_eq!(
        render_scope(|output| output.each(&oversized, |_, _| {
            calls.set(calls.get() + 1);
            Ok(())
        })),
        Err(RenderError::ItemLimit)
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(
        render_scope_with_limit(
            |output| {
                html! { output,
                    <ul>{|output| output.each(&["first", "second"], |output, item| {
                        calls.set(calls.get() + 1);
                        html! { output, <li>{item}</li> }
                    })}</ul>
                }
            },
            4
        ),
        Err(RenderError::OutputLimit)
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn one_macro_selects_owned_or_shared_output_without_ambient_context() {
    let calls = Cell::new(0);
    let shared = render_scope(|output| {
        html! { { calls . set (calls . get () + 1) ; output }, <p>"shared"</p> }
    })
    .unwrap();
    assert_eq!(calls.get(), 1);
    assert_eq!(shared.as_str(), "<p>shared</p>");
    assert_eq!(
        html! { "before" <b>"after"</b> }.unwrap().as_str(),
        "before<b>after</b>"
    );
    assert_eq!(
        html! { {"before"} <b>"after"</b> }.unwrap().as_str(),
        "before<b>after</b>"
    );
    let owned = html! { <p>"owned"</p> }.unwrap();
    assert_eq!(owned.as_str(), "<p>owned</p>");
}

#[test]
fn reusable_component_functions_need_no_forwarding_closure() {
    fn footer(output: &mut Output) -> RenderResult {
        html! { output, <footer>"end"</footer> }
    }
    let page = html! { <main>{rstml_ssr::component(footer)}</main> }.unwrap();
    assert_eq!(page.as_str(), "<main><footer>end</footer></main>");
}

#[test]
fn nested_owned_results_propagate_errors_instead_of_rendering_them() {
    for error in [
        RenderError::OutputLimit,
        RenderError::NullCharacter,
        RenderError::InvalidRawText,
    ] {
        let failed: RenderResult<Markup> = Err(error);
        assert_eq!(html! { <div>{failed}</div> }, Err(error));
    }
}
