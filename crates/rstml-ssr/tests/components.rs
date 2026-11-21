#![deny(warnings)]

use rstml_ssr::{
    Children, Markup, RenderError, RenderResult, component, html, render_scope_with_limit,
};
use std::cell::Cell;

mod shared {
    use super::*;

    #[component]
    pub fn Greeting(name: &str) -> RenderResult {
        let message = format!("Hello, {name}");
        html! { <p>{message}</p> }
    }

    #[component]
    pub fn Field(id: &str, label: &str, children: Children) -> RenderResult {
        html! {
            <div>
                <label for=id>{label}</label>
                {children()}
            </div>
        }
    }
}
use shared::{Field, Greeting};

struct PageMeta<'a> {
    title: &'a str,
    description: String,
}

#[component]
fn Page(meta: PageMeta<'_>, children: Children) -> RenderResult {
    let json = serde_json::json!({"description": meta.description})
        .to_string()
        .replace('<', "\\u003c");
    html! {
        <!DOCTYPE html>
        <html>
            <head>
                <title>{meta.title}</title>
                <script type="application/ld+json">{rstml_ssr::TrustedHtml(&json)}</script>
            </head>
            <body>{children()}</body>
        </html>
    }
}

#[component]
fn Consume(value: String) -> RenderResult {
    let bytes = value.into_bytes();
    let text = std::str::from_utf8(&bytes).unwrap();
    html! { <strong>{text}</strong> }
}

#[component]
fn Silent(children: Children) -> RenderResult {
    drop(children);
    html! { <hr /> }
}

#[component]
fn Maybe(show: bool, children: Children) -> RenderResult {
    html! { <aside>{if show { children() } else { "hidden" }}</aside> }
}

#[component]
fn Reject(error: RenderError) -> RenderResult {
    Err(error)
}

#[component]
fn Recursive() -> RenderResult {
    html! { <Recursive /> }
}

#[component]
fn Observe(expected: *const rstml_ssr::Output) -> RenderResult {
    html! {
        {|output| {
            assert_eq!(std::ptr::from_ref(output), expected);
            output.text("shared")
        }}
    }
}

#[component]
fn snake_name(mut name: String) -> RenderResult {
    name.push('!');
    html! { <p>{name}</p> }
}

#[component]
fn r#type(r#type: &str, r#children: Children) -> RenderResult {
    html! { <p>{r#type}{r#children()}</p> }
}

#[component]
#[cfg(any())]
fn Disabled(value: UndefinedType) -> RenderResult {
    undefined_function(value)
}

fn document(title: &str) -> RenderResult<Markup> {
    let description = String::from("</script><script>bad()</script>");
    html! {
        <Page meta=PageMeta { title, description }>
            <Field id="company" label="Company or project">
                <Greeting name=title />
            </Field>
            <shared::Greeting name="qualified" />
            <Consume value=String::from("once") />
        </Page>
    }
}

#[test]
fn named_properties_children_borrows_and_owned_locals_share_output() {
    let title = String::from("<Ada>");
    let page = document(&title).unwrap();
    drop(title);
    assert!(
        page.as_str()
            .starts_with("<!DOCTYPE html><html><head><title>&lt;Ada&gt;</title>")
    );
    assert!(page.as_str().contains(
        "<div><label for=\"company\">Company or project</label><p>Hello, &lt;Ada&gt;</p></div>"
    ));
    assert!(
        page.as_str()
            .contains("<p>Hello, qualified</p><strong>once</strong>")
    );
    assert_eq!(page.as_str().matches("<script").count(), 1);
}

#[test]
fn unused_children_are_not_evaluated_and_children_can_consume_values() {
    let calls = &Cell::new(0);
    let page = html! {
        <Silent>
            {{
                calls.set(calls.get() + 1);
                "unused"
            }}
        </Silent>
    }
    .unwrap();
    assert_eq!(calls.get(), 0);
    assert_eq!(page.as_str(), "<hr>");
    let value = String::from("owned");
    let page = html! {
        <Field id="one" label="One">
            <Consume value=value />
        </Field>
    }
    .unwrap();
    assert!(page.as_str().contains("<strong>owned</strong>"));
}

#[test]
fn children_work_in_rust_control_flow_and_writers_use_the_parent_buffer() {
    for show in [false, true] {
        let calls = &Cell::new(0);
        let page = html! {
            <Maybe show=show>
                {{
                    calls.set(calls.get() + 1);
                    "visible"
                }}
            </Maybe>
        }
        .unwrap();
        assert_eq!(calls.get(), u32::from(show));
        assert_eq!(
            page.as_str(),
            if show {
                "<aside>visible</aside>"
            } else {
                "<aside>hidden</aside>"
            }
        );
    }
    let page = rstml_ssr::render_scope(|output| {
        let expected = std::ptr::from_ref(output);
        html! { output,
            <Observe expected=expected />
            <SnakeName name=String::from("snake") />
        }
    })
    .unwrap();
    assert_eq!(page.as_str(), "shared<p>snake!</p>");
    assert_eq!(
        html! { <Type r#type="raw">"child"</Type> }
            .unwrap()
            .as_str(),
        "<p>rawchild</p>"
    );
}

#[test]
fn errors_stop_following_expressions_and_recursive_components_are_bounded() {
    for error in [
        RenderError::Allocation,
        RenderError::OutputLimit,
        RenderError::ItemLimit,
        RenderError::NullCharacter,
        RenderError::InvalidRawText,
        RenderError::InvalidAttributeName,
        RenderError::ComponentDepth,
    ] {
        let calls = Cell::new(0);
        assert_eq!(
            html! { <Reject error=error />{ { calls.set(1); "unreachable" } } },
            Err(error)
        );
        assert_eq!(calls.get(), 0);
    }
    let mut output = rstml_ssr::Output::new(1024);
    assert_eq!(
        output.scope(|output| html! { output, <Recursive /> }),
        Err(RenderError::ComponentDepth)
    );
    assert_eq!(output.scope(|_| Ok(())).unwrap(), "");
    assert_eq!(
        output
            .scope(|output| html! { output, <Greeting name="next" /> })
            .unwrap(),
        "<p>Hello, next</p>"
    );
}

#[test]
fn component_trees_obey_one_shared_byte_budget() {
    let expected = document("é<&>").unwrap();
    for limit in 0..=expected.as_str().len() as u32 + 1 {
        let result = render_scope_with_limit(
            |output| {
                html! { output,
                    <Page meta=PageMeta {
                        title: "é<&>",
                        description: String::from("</script><script>bad()</script>"),
                    }>
                        <Field id="company" label="Company or project">
                            <Greeting name="é<&>" />
                        </Field>
                        <shared::Greeting name="qualified" />
                        <Consume value=String::from("once") />
                    </Page>
                }
            },
            limit,
        );
        if limit < expected.as_str().len() as u32 {
            assert_eq!(result, Err(RenderError::OutputLimit));
        } else {
            assert_eq!(result.unwrap().as_str(), expected.as_str());
        }
    }
}

#[component]
fn Button(
    label: &str,
    #[prop(optional)] class: Option<&str>,
    #[prop(optional)] disabled: bool,
    #[prop(default = "button")] kind: &str,
) -> RenderResult {
    html! {
        <button type=kind class=class disabled=disabled>
            {label}
        </button>
    }
}

#[component]
fn Badge(#[prop(default = "info")] tone: &str) -> RenderResult {
    html! { <span data-tone=tone /> }
}

#[test]
fn optional_and_default_properties_apply_defaults_lazily() {
    let class = String::from("primary");
    let page = html! {
        <div>
            <Button label="Save" />
            <Button label="Submit" class={Some(class.as_str())} disabled kind="submit" />
            <Badge />
            <Badge tone="warn" />
        </div>
    }
    .unwrap();
    assert_eq!(
        page.as_str(),
        concat!(
            "<div>",
            "<button type=\"button\">Save</button>",
            "<button type=\"submit\" class=\"primary\" disabled=\"\">Submit</button>",
            "<span data-tone=\"info\"></span>",
            "<span data-tone=\"warn\"></span>",
            "</div>"
        )
    );
}
