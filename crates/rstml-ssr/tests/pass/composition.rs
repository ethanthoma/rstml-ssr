#![deny(warnings)]

use rstml_ssr::{Output, RenderResult, html, render_scope, write_html};

fn card(output: &mut Output, name: &str, title: Option<&str>) -> RenderResult {
    write_html! { output, <p title={title}>{name}</p> }
}

fn list(output: &mut Output, names: &[String]) -> RenderResult {
    write_html! { output,
        <ul>
            {|output| {
                output
                    .each(
                        names,
                        |output, name| {
                            write_html! { output, <li>{name}</li> }
                        },
                    )
            }}
        </ul>
    }
}

fn shell(output: &mut Output, body: impl FnOnce(&mut Output) -> RenderResult) -> RenderResult {
    write_html! { output,
        <!DOCTYPE html>
        <html>
            <head>
                <title>"Test"</title>
            </head>
            <body>{|output| body(output)}</body>
        </html>
    }
}

fn main() {
    let name = String::from("Ada");
    let card = render_scope(|output| card(output, &name, Some("Title"))).unwrap();
    assert_eq!(card.as_str(), "<p title=\"Title\">Ada</p>");
    let names = [name];
    let page = render_scope(|output| shell(output, |output| list(output, &names))).unwrap();
    assert!(page.as_str().contains("<li>Ada</li>"));
    let selected = Some(String::from("owned"));
    let page = html! {
        {if let Some(value) = selected {
            html! { <b>{value}</b> }
        } else {
            "empty"
        }}
    }.unwrap();
    assert_eq!(page.as_str(), "<b>owned</b>");
}
