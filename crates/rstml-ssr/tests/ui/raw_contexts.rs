use rstml_ssr::html;

fn main() {
    let _ = html! { <script>{"alert(1)"}</script> };
    let _ = html! { <xmp/> };
    let _ = html! { <iframe/> };
    let _ = html! { <noembed/> };
    let _ = html! { <noframes/> };
    let _ = html! { <plaintext/> };
    let _ = html! { <div onclick={"alert(1)"}/> };
    let _ = html! { <div style={"color: red"}/> };
    let _ = html! { <style>{String::from("p { color: red; }")}</style> };
    let _ = html! { <script><div/></script> };
    let _ = html! { <svg><script>{"alert(1)"}</script></svg> };
    let _ = html! { <math><annotation-xml encoding={"text/html"}/></math> };
}
