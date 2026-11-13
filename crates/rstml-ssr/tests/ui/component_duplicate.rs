use rstml_ssr::{component, html, Children, RenderResult};

#[component]
fn Field(id: &str, children: Children) -> RenderResult {
    html! { <div id=id>{children()}</div> }
}

fn main() {
    let _ = html! { <Field id="one" id="two">"child"</Field> };
    let _ = html! { <Field id="one" children={|_| Ok(())}>"child"</Field> };
}
