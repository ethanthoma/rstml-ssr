#![deny(warnings)]

use rstml_ssr::{Render, each, html, render};

fn card<'a>(name: &'a str, title: Option<&'a str>) -> impl Render + 'a {
    html! { <p title={title}>{name}</p> }
}

fn list(names: &[String]) -> impl Render + '_ {
    html! { <ul>{each(names, |name| html! { <li>{name}</li> })}</ul> }
}

fn shell(content: impl Render) -> impl Render {
    html! {
        <!DOCTYPE html>
        <html>
            <head>
                <title>"Test"</title>
            </head>
            <body>{content}</body>
        </html>
    }
}

fn main() {
    let name = String::from("Ada");
    let card = card(&name, Some("Title"));
    assert_eq!(render(&card).unwrap().as_str(), "<p title=\"Title\">Ada</p>");
    drop(card);
    let names = [name];
    assert!(render(&shell(list(&names))).unwrap().as_str().contains("<li>Ada</li>"));
    let selected = Some(String::from("owned"));
    let page = html! {
        {if let Some(value) = selected {
            html! { <b>{value}</b> }
        } else {
            "empty"
        }}
    };
    assert_eq!(render(&page).unwrap().as_str(), "<b>owned</b>");
}
