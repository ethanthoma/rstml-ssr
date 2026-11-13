use rstml_ssr::{
    Markup, RenderError, RenderResult, TrustedHtml, component, html, render, render_with_limit,
};

fn greeting(name: String) -> RenderResult<Markup> {
    html! { <p title={name}>{name}{name}</p> }
}

#[test]
fn literal_markup_and_void_elements() {
    let page = html! {
        <!DOCTYPE html>
        <html><head><meta charset="utf-8"/><link rel="stylesheet" href="/main.css"/></head>
        <body><><custom-element xml:lang="en"/><div/><br><input required disabled=false/></></body></html>
    };
    assert_eq!(
        render(&page).unwrap().as_str(),
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><link rel=\"stylesheet\" href=\"/main.css\"></head><body><custom-element xml:lang=\"en\"></custom-element><div></div><br><input required=\"\"></body></html>"
    );
    assert_eq!(render(&html! {}).unwrap().as_str(), "");
}

#[test]
fn interpolation_is_reusable_and_evaluated_once() {
    let calls = std::cell::Cell::new(0);
    let page = html! {
        <div title={{
            calls.set(calls.get() + 1);
            "\"<&>"
        }}>"<&>\"" {greeting(String::from("<world>"))} {42_u32} {TrustedHtml("<svg></svg>")}</div>
    };
    assert_eq!(calls.get(), 1);
    let expected = "<div title=\"&quot;&lt;&amp;&gt;\">&lt;&amp;&gt;\"<p title=\"&lt;world&gt;\">&lt;world&gt;&lt;world&gt;</p>42<svg></svg></div>";
    assert_eq!(render(&page).unwrap().as_str(), expected);
    assert_eq!(render(&page).unwrap().as_str(), expected);
    assert_eq!(calls.get(), 1);
    assert_eq!(render_with_limit(&page, 10), Err(RenderError::OutputLimit));
}

#[test]
fn rust_branches_can_return_different_fragment_types() {
    for selected in 0..4 {
        let page = html! {
            {if selected == 0 {
                html! { <p>"zero"</p> }
            } else if selected == 1 {
                html! { <strong>{1_u32}</strong> }
            } else {
                "other"
            }}
            {match selected {
                0 => html! { <b /> },
                value if value == 1 => html! { <i>{value}</i> },
                2 => "two",
                _ => 3_u32,
            }}
        };
        let expected = [
            "<p>zero</p><b></b>",
            "<strong>1</strong><i>1</i>",
            "othertwo",
            "other3",
        ];
        assert_eq!(render(&page).unwrap().as_str(), expected[selected]);
    }
}

#[test]
fn optional_and_collected_content_preserves_order_and_escaping() {
    for visible in [false, true] {
        let optional = visible.then(|| html! { <b>"<&>"</b> });
        let items = ["one", "<two>"]
            .iter()
            .map(|name| html! { <li>{name}</li> })
            .collect::<Vec<_>>();
        let page = html! {
            {if visible {
                html! { <p>"shown"</p> }
            }}
            {if let Some(value) = optional {
                value
            }}
            <ul>{items}</ul>
        };
        let expected = if visible {
            "<p>shown</p><b>&lt;&amp;&gt;</b><ul><li>one</li><li>&lt;two&gt;</li></ul>"
        } else {
            "<ul><li>one</li><li>&lt;two&gt;</li></ul>"
        };
        assert_eq!(render(&page).unwrap().as_str(), expected);
        assert_eq!(render(&page).unwrap().as_str(), expected);
        assert_eq!(render_with_limit(&page, 1), Err(RenderError::OutputLimit));
    }
    assert_eq!(render(&None::<String>).unwrap().as_str(), "");
    assert_eq!(render(&Some("<&>")).unwrap().as_str(), "&lt;&amp;&gt;");
    assert_eq!(render(&Some("\0")), Err(RenderError::NullCharacter));
    let items = vec![(); rstml_ssr::ITEM_COUNT_MAX as usize + 1];
    assert_eq!(render(&items), Err(RenderError::ItemLimit));
    assert_eq!(render(items.as_slice()), Err(RenderError::ItemLimit));
    assert_eq!(
        render(&items[..rstml_ssr::ITEM_COUNT_MAX as usize])
            .unwrap()
            .as_str(),
        ""
    );
}

#[test]
fn lazy_content_evaluates_only_selected_branches_at_render_time() {
    let selected = std::cell::Cell::new(false);
    let calls = std::cell::Cell::new(0_u32);
    let page = component(|output| {
        html! { output,
            {|| {
                if selected.get() {
                    calls.set(calls.get() + 1);
                    html! { <b>"selected"</b> }
                } else {
                    "fallback"
                }
            }}
        }
    });
    assert_eq!(calls.get(), 0);
    assert_eq!(render(&page).unwrap().as_str(), "fallback");
    assert_eq!(calls.get(), 0);
    selected.set(true);
    assert_eq!(render(&page).unwrap().as_str(), "<b>selected</b>");
    assert_eq!(calls.get(), 1);
    assert_eq!(render(&page).unwrap().as_str(), "<b>selected</b>");
    assert_eq!(calls.get(), 2);
    assert_eq!(render_with_limit(&page, 1), Err(RenderError::OutputLimit));
    let callback = || "\0";
    assert_eq!(
        render(&rstml_ssr::lazy(callback)),
        Err(RenderError::NullCharacter)
    );
}

#[test]
fn typed_lazy_closures_preserve_rust_return_types() {
    let selected = std::cell::Cell::new(false);
    let calls = std::cell::Cell::new(0_u32);
    let page = component(|output| {
        html! { output,
            {|| -> &'static str {
                calls.set(calls.get() + 1);
                if selected.get() { "<&>" } else { "hidden" }
            }}
            {|| -> &'static str {
                match selected.get() {
                    true => "shown",
                    false => "fallback",
                }
            }}
        }
    });
    assert_eq!(calls.get(), 0);
    for (index, visible) in [false, true, false].into_iter().enumerate() {
        selected.set(visible);
        let expected = if visible {
            "&lt;&amp;&gt;shown"
        } else {
            "hiddenfallback"
        };
        assert_eq!(render(&page).unwrap().as_str(), expected);
        assert_eq!(calls.get(), index as u32 + 1);
    }
    assert_eq!(render_with_limit(&page, 1), Err(RenderError::OutputLimit));
    let invalid = html! { {|| -> &'static str { if selected.get() { "valid" } else { "\0" } }} };
    assert_eq!(render(&invalid), Err(RenderError::NullCharacter));
}

#[test]
fn datastar_names_omission_and_multiline_text() {
    let expression = "@post('/save', {body: 'a\"<&>b'})";
    let page = html! {
        <form
            id="profile"
            data-on:submit={expression}
            data-attr:disabled="$busy"
            aria-label={Some("")}
            hidden={false}
            required={true}
            title={None::<&str>}
        >
            "first\nsecond"
            <button type="submit">"Save"</button>
        </form>
    };
    assert_eq!(
        render(&page).unwrap().as_str(),
        "<form id=\"profile\" data-on:submit=\"@post('/save', {body: 'a&quot;&lt;&amp;&gt;b'})\" data-attr:disabled=\"$busy\" aria-label=\"\" required=\"\">first\nsecond<button type=\"submit\">Save</button></form>"
    );
}

#[test]
fn lists_share_the_output_budget_and_bound_empty_items() {
    use rstml_ssr::{ITEM_COUNT_MAX, each};
    let items = ["<one>", "two"];
    let page = html! { <ul>{each(&items, |item| html! { <li>{item}</li> })}</ul> };
    assert_eq!(
        render(&page).unwrap().as_str(),
        "<ul><li>&lt;one&gt;</li><li>two</li></ul>"
    );
    assert_eq!(render_with_limit(&page, 10), Err(RenderError::OutputLimit));
    assert_eq!(
        render(&each(&[] as &[u32], |item| html! { {item} }))
            .unwrap()
            .as_str(),
        ""
    );
    let empty_items = vec![(); ITEM_COUNT_MAX as usize + 1];
    let calls = std::cell::Cell::new(0);
    let list = each(&empty_items, |_| {
        calls.set(calls.get() + 1);
        ""
    });
    assert_eq!(render(&list), Err(RenderError::ItemLimit));
    assert_eq!(calls.get(), 0);
    assert_eq!(
        render(&each(&empty_items[..ITEM_COUNT_MAX as usize], |_| ""))
            .unwrap()
            .as_str(),
        ""
    );
}

#[test]
fn static_and_dynamic_values_use_the_same_escaping_contract() {
    let value = "\"'é<&>\n";
    let literal = html! { <p title="\"'é<&>\n">"\"'é<&>\n"</p> };
    let dynamic = html! { <p title={value}>{value}</p> };
    assert_eq!(render(&literal), render(&dynamic));
}

#[test]
fn generated_bindings_do_not_shadow_caller_names() {
    let __rstml_output = String::from("caller");
    let __rstml_value_0 = 42_u32;
    assert_eq!(
        render(&html! { <p>{__rstml_output}{__rstml_value_0}</p> })
            .unwrap()
            .as_str(),
        "<p>caller42</p>"
    );
}

#[test]
fn external_scripts_have_escaped_attributes_and_no_inline_content() {
    let url = "/static/js/form-drafts.js?v=one&module=\"two\"";
    let page = html! {
        <script type="module" src={url} />
        <noscript>
            <p>"JavaScript is required to verify and submit this form."</p>
        </noscript>
    };
    assert_eq!(
        render(&page).unwrap().as_str(),
        "<script type=\"module\" src=\"/static/js/form-drafts.js?v=one&amp;module=&quot;two&quot;\"></script><noscript><p>JavaScript is required to verify and submit this form.</p></noscript>"
    );
}
