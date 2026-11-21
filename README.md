# rstml-ssr

[![CI](https://github.com/ethanthoma/rstml-ssr/actions/workflows/ci.yml/badge.svg)](https://github.com/ethanthoma/rstml-ssr/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/rstml-ssr.svg)](https://crates.io/crates/rstml-ssr)
[![docs.rs](https://docs.rs/rstml-ssr/badge.svg)](https://docs.rs/rstml-ssr)

Server-side HTML for Rust. Leptos-style components write into one bounded
buffer, without a browser runtime or view tree.

```toml
[dependencies]
rstml-ssr = "0.4"
```

## Usage

```rust
use rstml_ssr::{Children, RenderResult, component, html};

#[component]
fn Greeting(name: &str) -> RenderResult {
    let message = format!("Hello, {name}");
    html! { <p>{message}</p> }
}

#[component]
fn Field(label: &str, children: Children) -> RenderResult {
    html! {
        <section>
            <h2>{label}</h2>
            {children()}
        </section>
    }
}

fn main() -> RenderResult {
    let name = String::from("Ada");
    let page = html! {
        <Field label="Company">
            <Greeting name=&name />
        </Field>
    }?;
    println!("{}", page.as_str());
    Ok(())
}
```

Outside a component, `html!` returns owned `RenderResult<Markup>`. Inside
`#[component]`, it writes immediately into the caller's output. Explicit
`html! { output, ... }` and ordinary writer functions remain available.

Properties use Rust's move and borrowing rules. `Children` becomes an unboxed
`FnOnce` writer. Children execute when called, not when supplied. Missing,
unknown, or mistyped properties fail at compile time.

Required concrete properties, `#[prop(optional)]`, `#[prop(default = ...)]`, and
lifetime parameters are supported. Type/const generics are not yet supported.
PascalCase tags are components, lowercase tags are HTML. Keep SVG/MathML roots
inside components, without children slots inside those roots.

## Safety

- Text and attributes are escaped. URL schemes, JavaScript, CSS, and Datastar
  expressions still require application validation.
- `TrustedHtml` is for audited markup only. For JSON-LD, escape `<` as `\u003c`
  before wrapping serialized JSON.
- Rendering defaults to 1 MiB, 64 nested component/callback calls, and 65,536
  items per list. `render_scope_with_limit` changes the byte budget.
- Propagate write errors. Failed owning scopes discard partial output. Render
  before sending HTTP headers or SSE events.
- Rendering does not roll back database writes or mail delivery. Keep those
  outside components.

## Development

```console
nix develop
cargo test --workspace
nix flake check
```

Format templates with Leptosfmt, then run `cargo fmt --all`.

MIT OR Apache-2.0. See [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).
