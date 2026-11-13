#![deny(warnings)]
#![no_implicit_prelude]

extern crate core;
extern crate renamed;
extern crate std;

mod forwarded {
    pub use crate::renamed::{component as declare, html as template, html as append};
}

fn greeting(name: &str) -> renamed::RenderResult<renamed::Markup> {
    forwarded::template! { <p title={name}>{name}</p> }
}

fn component(output: &mut renamed::Output, name: &str) -> renamed::RenderResult {
    forwarded::append! { output, {greeting(name)} }
}

#[forwarded::declare]
fn RenamedComponent(title: &str, children: renamed::Children) -> renamed::RenderResult {
    renamed::html! { <aside>{title}{children()}</aside> }
}

fn main() {
    let page = renamed::html! {
        <main>
            {|output| component(output, "Ada")}
            {if true { renamed::html! { <b>"yes"</b> } } else { "no" }}
            {match 1_u32 { 0 => renamed::html! { <i/> }, 1 => "one", _ => 2_u32 }}
            <RenamedComponent title="renamed"><b>"child"</b></RenamedComponent>
        </main>
    }.unwrap();
    std::assert_eq!(
        page.as_str(),
        "<main><p title=\"Ada\">Ada</p><b>yes</b>one<aside>renamed<b>child</b></aside></main>"
    );
}
