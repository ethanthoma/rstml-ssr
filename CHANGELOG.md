# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Enforce a 64-level element nesting depth limit in `html!` and `#[component]`.
- Combine compile errors across multiple failing `html!` invocations inside a
  single `#[component]` instead of discarding earlier diagnostics.
- Align `Output::render`, `render`, and `render_with_limit` signatures to use
  `RenderResult`.
- Migrate property-based tests to Hegel (`hegeltest`).
- Migrate `flake.nix` to `flake-parts` and `numtide/devshell`, include Clippy,
  rustdoc, and `taplo` in `nix flake check`, and commit formatter configs.

## [0.4.0] - 2026-11-13

### Added

- Add Leptos-style `#[component]` writer components with named, type-checked
  properties, borrowed lifetimes, and one-shot `Children` slots.
- Support PascalCase component tags inside `html!` templates.

### Changed

- Inside `#[component]`, `html!` writes directly into the caller's rendering
  scope without repeating the output binding.

## [0.3.0] - 2026-11-13

### Changed

- Unify owned and scoped rendering under the single `html!` macro:
  `html! { ... }` returns `RenderResult<Markup>` and `html! { output, ... }`
  appends to an existing `Output`.

## [0.2.0] - 2026-11-13

### Added

- Add `Output::scope`, `Output::component`, `Output::each`, `render_scope`, and
  `render_scope_with_limit` for shared-buffer component rendering.

### Changed

- Render owned `Markup` directly in component scopes with a single shared byte
  budget.

## [0.1.1] - 2026-11-12

### Fixed

- Unwrap single-expression macro tail blocks in `html!` without emitting
  unused-braces warnings.

## [0.1.0] - 2026-11-12

### Added

- Initial server-side HTML rendering crate (`rstml-ssr`) and proc-macro crate
  (`rstml-ssr-macros`) with escaped text, typed attributes, bounded output
  budgets, `TrustedHtml`, SVG/MathML namespace handling, and raw-text guards.
- Add CI and tagged crates.io / GitHub release workflows.

[0.1.0]: https://github.com/ethanthoma/rstml-ssr/releases/tag/v0.1.0
[0.1.1]: https://github.com/ethanthoma/rstml-ssr/compare/v0.1.0...v0.1.1
[0.2.0]: https://github.com/ethanthoma/rstml-ssr/compare/v0.1.1...v0.2.0
[0.3.0]: https://github.com/ethanthoma/rstml-ssr/compare/v0.2.0...v0.3.0
[0.4.0]: https://github.com/ethanthoma/rstml-ssr/compare/v0.3.0...v0.4.0
[unreleased]: https://github.com/ethanthoma/rstml-ssr/compare/v0.4.0...HEAD
