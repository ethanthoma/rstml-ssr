use rstml_ssr::html;

fn main() {
    let _ = html! { <{"div"}/> };
    let _ = html! { <div {"title"}="value"/> };
    let _ = html! { <div {"title"}/> };
}
