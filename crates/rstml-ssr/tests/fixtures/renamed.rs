#![deny(warnings)]
#![no_implicit_prelude]

extern crate core;
extern crate renamed;
extern crate std;

mod forwarded {
    pub use crate::renamed::html as template;
}

fn greeting(name: &str) -> impl renamed::Render + '_ {
    forwarded::template! { <p title={name}>{name}</p> }
}

fn main() {
    let page = renamed::html! {
        <main>
            {greeting("Ada")}
            {if true { renamed::html! { <b>"yes"</b> } } else { "no" }}
            {match 1_u32 { 0 => renamed::html! { <i/> }, 1 => "one", _ => 2_u32 }}
        </main>
    };
    std::assert_eq!(
        renamed::render(&page).unwrap().as_str(),
        "<main><p title=\"Ada\">Ada</p><b>yes</b>one</main>"
    );
}
