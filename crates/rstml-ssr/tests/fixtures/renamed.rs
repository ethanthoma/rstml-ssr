#![deny(warnings)]
#![no_implicit_prelude]

extern crate core;
extern crate renamed;
extern crate std;

mod forwarded {
    pub use crate::renamed::{html as template, write_html as append};
}

fn greeting(name: &str) -> renamed::RenderResult<renamed::Markup> {
    forwarded::template! { <p title={name}>{name}</p> }
}

fn component(output: &mut renamed::Output, name: &str) -> renamed::RenderResult {
    forwarded::append! { output, {greeting(name)} }
}

fn main() {
    let page = renamed::html! {
        <main>
            {|output| component(output, "Ada")}
            {if true { renamed::html! { <b>"yes"</b> } } else { "no" }}
            {match 1_u32 { 0 => renamed::html! { <i/> }, 1 => "one", _ => 2_u32 }}
        </main>
    }.unwrap();
    std::assert_eq!(
        page.as_str(),
        "<main><p title=\"Ada\">Ada</p><b>yes</b>one</main>"
    );
}
