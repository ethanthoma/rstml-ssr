use rstml_ssr::{component, html, RenderResult};

#[component]
fn Greeting(name: &str) -> RenderResult {
    html! { <p>{name}</p> }
}

fn main() {
    let _ = html! { <Greeting /> };
    let _ = html! { <Greeting name=42_u32 /> };
    let _ = html! { <Greeting name="Ada" unknown=true /> };
    let _ = html! { <UnknownComponent /> };
}
