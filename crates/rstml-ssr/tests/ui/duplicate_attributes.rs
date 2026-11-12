use rstml_ssr::html;

fn main() {
    let dynamic = "warning";
    let _ = html! { <div class="alert" class={dynamic}/> };
    let _ = html! { <div CLASS="alert" class={dynamic}/> };
    let _ = html! { <input disabled={false} disabled={true}/> };
    let _ = html! { <svg viewBox="0 0 1 1" viewbox="0 0 2 2"/> };
    let _ = html! { <div data-on:click="first" data-on:click="second"/> };
}
