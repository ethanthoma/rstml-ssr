use rstml_ssr::html;

fn main() {
    let _ = html! { <INPUT>"child"</INPUT> };
    let _ = html! { <div></span> };
    let _ = html! { <div>unquoted text</div> };
    let _ = html! { <!DOCTYPE svg> };
    let _ = html! { <div title(value)/> };
}

fn nul_literals() {
    let _ = html! { <p>"\0"</p> };
    let _ = html! { <p title="\0"/> };
}
