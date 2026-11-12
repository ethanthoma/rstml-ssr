use rstml_ssr::{html, render, TrustedHtml};

fn main() {
    let _ = html! { <p>{std::net::Ipv4Addr::LOCALHOST}</p> };
    let _ = html! { <p title={TrustedHtml("bad")}/> };
    let _ = html! { <p title={render(&"bad").unwrap()}/> };
    let _ = html! { <p data-state={serde_json::json!({"name": "value"})}/> };
}
