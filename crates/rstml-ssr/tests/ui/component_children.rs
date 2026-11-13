use rstml_ssr::{component, html, Children, RenderResult};

#[component]
fn Twice(children: Children) -> RenderResult {
    html! { <div>{children()}{children()}</div> }
}

fn main() {
    let _ = html! { <Twice>"one"</Twice> };
}
