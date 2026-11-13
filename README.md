# rstml-ssr

[![CI](https://github.com/ethanthoma/rstml-ssr/actions/workflows/ci.yml/badge.svg)](https://github.com/ethanthoma/rstml-ssr/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/rstml-ssr.svg)](https://crates.io/crates/rstml-ssr)
[![docs.rs](https://docs.rs/rstml-ssr/badge.svg)](https://docs.rs/rstml-ssr)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)

Server-side HTML templates for Rust, using
[rstml](https://github.com/rs-tml/rstml) syntax and ordinary Rust expressions.

A rendering scope owns the output buffer. Components execute within that scope,
borrowing their local values while writing. The completed document owns its
bytes. No browser runtime is required.

## Installation

```toml
[dependencies]
rstml-ssr = "0.3"
```

## Owned HTML

`html!` renders immediately and returns `RenderResult<Markup>`:

```rust
use rstml_ssr::{RenderResult, html};

fn main() -> RenderResult {
    let name = String::from("<visitor>");
    let page = html! {
        <p title={name}>"Hello, "{name}</p>
    }?;
    drop(name);
    assert_eq!(page.as_str(), "<p title=\"&lt;visitor&gt;\">Hello, &lt;visitor&gt;</p>");
    Ok(())
}
```

Interpolations are borrowed while writing. There are no expression-capture
heuristics or retained template borrows. Rust expressions execute in document
order. A write error stops subsequent expressions.

Use `as_str()` to borrow the completed HTML or `into_string()` to take it.
Nested `RenderResult` values propagate their errors rather than rendering error
messages.

## Components and shared output

A component is an ordinary function taking `&mut Output` and returning
`RenderResult`. The `html! { output, ... }` form appends to that output instead
of allocating a child document. A one-argument content closure invokes a child
component in the same scope:

```rust
use rstml_ssr::{Output, RenderResult, render_scope, html};

fn greeting(output: &mut Output, name: &str) -> RenderResult {
    let message = format!("Hello, {name}");
    html! { output, <p>{message}</p> }
}

fn card(output: &mut Output, name: &str) -> RenderResult {
    html! { output,
        <section>{|output| greeting(output, name)}</section>
    }
}

fn main() -> RenderResult {
    let page = render_scope(|output| card(output, "<visitor>"))?;
    assert_eq!(page.as_str(), "<section><p>Hello, &lt;visitor&gt;</p></section>");
    Ok(())
}
```

Callbacks accept `FnOnce`, so components can consume owned values. Components
must propagate write errors. Keep mail delivery, database writes, and other
application mutations outside rendering. Discarding output does not roll back
those operations.

Use `Output::each(items, |output, item| ...)` for bounded lists sharing the same
buffer and byte budget. Oversized lists fail before invoking callbacks. Rust
statements and control flow can live directly in component functions or child
callbacks.

An owned child from `html!` can also be interpolated, but its bytes are copied
into the parent. Use shared-output components when that intermediate document is
unnecessary. Both forms share the same code generator and validation rules. The
output destination is explicit, not inferred from variable names or return
types.

For a component taking only the output, use `{component(footer)}` rather than
`{|output| footer(output)}`. The `component` adapter implements `Render` for a
reusable writer function. Inline one-argument callbacks also support `FnOnce`
captures.

## Expressions and trusted content

Content expressions support `if`, `if let`, and `match`, including branches with
different renderable types. An omitted `else`, `None`, or `()` renders nothing.
Slices and vectors render in order. Explicit zero-argument callbacks and `lazy`
remain available for low-level reusable `Render` fragments. Ordinary `html!`
output is an eager snapshot, not a live view.

- Text and double-quoted attribute values are escaped separately.
- `false` and `None` omit an attribute. `true` emits an empty attribute; empty
  strings remain present.
- Duplicate attribute names are compile errors, including case variants.
- SVG, MathML, custom elements, and Datastar attributes are supported.
- Dynamic names, attribute spreading, and dynamic native event-handler or
  `style` attributes are rejected.

`TrustedHtml` bypasses escaping. Use it only for audited markup, never untrusted
input. It cannot be used as an attribute value. Local generated strings can be
borrowed while a component writes them:

```rust
use rstml_ssr::{Output, RenderResult, TrustedHtml, html};

fn audited_svg(output: &mut Output, source: String) -> RenderResult {
    html! { output, <div>{TrustedHtml(&source)}</div> }
}
```

Inline script and style bodies accept quoted source or `TrustedHtml` values.
HTML raw-text bodies reject NUL and tokenizer control sequences, including
sequences split across interpolations. For JSON-LD, serialize the JSON and
escape `<` as `\u003c` before wrapping it in `TrustedHtml`. Other raw-text
elements, such as `iframe`, require a complete `TrustedHtml` block.

Validate URL schemes, JavaScript, CSS, and Datastar expressions in the
application. HTML escaping does not validate those languages.

## Errors and limits

Handle the outer `RenderResult<Markup>` before constructing HTTP headers or SSE
events. A failed `html!` or `render_scope` returns no document. Child components
share the scope's byte budget. Rendering reports output limits, allocation
failures, invalid attribute names, NUL, and invalid raw-text content.

The default output limit is 1 MiB. `render_scope_with_limit` accepts an explicit
UTF-8 byte budget. Slices, vectors, and `Output::each` accept at most 65,536
items per list.

`Output::scope` replaces a document while retaining buffer capacity. Errors
clear partial output. Use it only at document boundaries, not inside child
components. Low-level writes through `html!`, `Output::component`, or
`Render::render` can leave partial output on error. Propagate those errors to
the owning scope before reusing the buffer.

`Render` is the low-level mechanism for appending values and completed markup.
`render` and `render_with_limit` materialize such values into owned documents.

## Moving from 0.2

Use `html! { output, ... }` in place of `write_html!`. The separate macro is
removed. The reusable `fragment` adapter is named `component`.

## Moving from 0.1

- `html!` now returns `RenderResult<Markup>`, not a reusable component.
- Change owned view helpers to return `RenderResult<Markup>`. Their returned
  markup no longer needs lifetime parameters.
- Use component functions, `html!`, and `render_scope` to compose without
  intermediate buffers or `component(move |output| ...)` wrappers.
- Repeated use of completed markup repeats the same bytes. To reevaluate data,
  execute the component again in a new scope.

## Development

```console
nix develop
cargo test --workspace
nix flake check
```

Leptosfmt formats both forms of `html!`. Run it with `leptosfmt.toml`, then run
`cargo fmt --all`. Nix checks enforce both formatters and structural lint rules.

Release tags must match the workspace version and point to a commit on `main`.
The release workflow runs CI, publishes the macro crate before the runtime, then
creates a GitHub release. Publishing uses the repository's
`CARGO_REGISTRY_TOKEN` Actions secret.

## License

Licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option.
