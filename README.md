# rstml-ssr

[![CI](https://github.com/ethanthoma/rstml-ssr/actions/workflows/ci.yml/badge.svg)](https://github.com/ethanthoma/rstml-ssr/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/rstml-ssr.svg)](https://crates.io/crates/rstml-ssr)
[![docs.rs](https://docs.rs/rstml-ssr/badge.svg)](https://docs.rs/rstml-ssr)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

Server-side HTML templates for Rust, using
[rstml](https://github.com/rs-tml/rstml) syntax and ordinary Rust expressions.

Templates produce reusable fragments. Nested fragments write into one output
buffer, with text and attribute escaping handled by the renderer. No browser
runtime is required.

## Installation

```toml
[dependencies]
rstml-ssr = "0.1"
```

## Render HTML

```rust
use rstml_ssr::{RenderError, html, render};

fn main() -> Result<(), RenderError> {
    let name = "<visitor>";
    let page = html! {
        <p title={name}>"Hello, "{name}</p>
    };

    println!("{}", render(&page)?.as_str());
    Ok(())
}
```

Output:

```html
<p title="&lt;visitor&gt;">Hello, &lt;visitor&gt;</p>
```

`html!` returns a fragment implementing `Render`. `render` turns it into owned
`Markup`; use `as_str()` to borrow the HTML or `into_string()` to take it.

## Compose templates

Use Rust functions for reusable markup and `each` to render a slice without
collecting intermediate fragments:

```rust
use rstml_ssr::{Render, each, html};

fn names_list(names: &[String]) -> impl Render + '_ {
    html! {
        <section>
            {if names.is_empty() {
                html! { <p>"No names yet."</p> }
            }}
            <ul>{each(names, |name| html! { <li>{name}</li> })}</ul>
        </section>
    }
}
```

Content expressions support `if`, `if let`, and `match`, including branches with
different renderable types. An omitted `else`, `None`, or `()` renders nothing.
Slices and vectors of renderable values render in order.

Expressions are evaluated when the fragment is constructed. A zero-argument
closure inside `{...}` runs on each render; use `lazy(callback)` for a named
callback. Closures must implement `Fn`. Fragments follow Rust ownership and
borrowing rules.

## Attributes and trusted content

- Text and double-quoted attribute values are escaped separately.
- `false` and `None` omit an attribute. `true` emits an empty attribute; empty
  strings remain present.
- Duplicate attribute names are compile errors, including case variants.
- SVG and MathML can be written directly. Custom element names and Datastar
  attributes such as `data-on:submit` are supported.
- Dynamic names, attribute spreading, and dynamic native event-handler or
  `style` attributes are rejected.

`TrustedHtml` bypasses escaping. Use it only for audited markup, never untrusted
input. It cannot be used as an attribute value.

Inline script and style bodies accept quoted source or `TrustedHtml` values.
HTML raw-text bodies reject NUL and tokenizer control sequences, including
sequences split across interpolations. For JSON-LD, serialize the JSON and
escape `<` as `\u003c` before wrapping it in `TrustedHtml`.

Other raw-text elements, such as `iframe`, require a complete `TrustedHtml`
block.

Validate URL schemes, JavaScript, CSS, and Datastar expressions in the
application; HTML escaping does not validate those languages.

## Errors and limits

Rendering returns `Result<Markup, RenderError>`. Handle errors at the response
boundary; template helpers can return `impl Render` without handling errors at
every level.

The default output limit is 1 MiB. Use `render_with_limit` to choose a different
byte budget. Slices, vectors, and `each` accept at most 65,536 items per list.
Rendering also reports allocation failures, invalid attribute names, NUL, and
invalid raw-text content.

`Output::render` reuses buffer capacity across documents and returns a borrowed
string. It clears the previous document and clears partial output on error.
Low-level writes through `Render::render` can leave partial output after an
error; discard that content before reusing the buffer.

## Development

```console
nix develop
cargo test --workspace
nix flake check
```

Template formatting uses `leptosfmt` alongside `rustfmt`. Nix checks enforce
formatting and structural lint rules.

Release tags must match the workspace version, for example `v0.1.0`, and point
to a commit on `main`. The release workflow runs CI, publishes the macro crate
before the runtime, then creates a GitHub release. Publishing uses the
repository's `CARGO_REGISTRY_TOKEN` Actions secret. On failure, use GitHub's
"Re-run failed jobs" option to keep successful jobs complete.

## License

Licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option.
