use rstml_ssr::{component, html, RenderResult};

#[component]
fn Path() -> RenderResult { html! { <path d="M0 0L1 1" /> } }

#[component]
fn Foreign(children: rstml_ssr::Children) -> RenderResult {
    html! { <svg>{children()}</svg> }
}

fn main() {
    let _ = html! { <svg><Path /></svg> };
}
