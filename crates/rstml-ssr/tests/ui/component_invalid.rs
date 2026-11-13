use rstml_ssr::component;

#[component]
async fn Async() -> rstml_ssr::RenderResult { Ok(()) }

#[component]
fn Generic<T>(value: T) -> rstml_ssr::RenderResult { let _ = value; Ok(()) }

#[component]
fn Optional(#[prop(optional)] value: Option<String>) -> rstml_ssr::RenderResult { let _ = value; Ok(()) }

#[component]
fn Reserved(build: bool) -> rstml_ssr::RenderResult { let _ = build; Ok(()) }

fn main() {}
