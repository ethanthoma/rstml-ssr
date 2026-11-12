# Website fixture evidence

The fixtures adapt the local `website` repository at revision
`f8cf6b5fbe184dda97292b7321b78c61ce5c5b4d`:

- `src/views.rs`: shared shell, head, navigation, footer, CAPTCHA helpers, SVG.
- `src/account_views.rs`: contact form, field, and required-input helpers.
- `assets/imgs/matter-state.svg`: the frozen institution mark.
- `src/web.rs::patch_event`: the LF-based `elements` framing algorithm.

The website is not a build dependency and was not changed. Its asset resolver is
replaced by deterministic fixture URLs. The CAPTCHA configuration is frozen at
its serialized value. Schema markup is supplied as a complete audited block
instead of importing the website's JSON-building dependencies. The original Maud
template structure and attributes are retained. The rstml version renders the
same contact form and shared shell, without the surrounding contact-page content
or server integrations.

Comparisons use Maud 0.27.0, revision
`fb193d31462eea63eabfa705c93ccc56f5bb1eac`, and scraper 0.29.0, revision
`149ae5c9dfbaca2e6b92470018fedc95ab254d5d` (html5ever for HTML5 parsing). Maud,
scraper, trybuild, serde_json, and tiny_http are test dependencies only.

The form and eight shell variants produce the same parsed HTML. The shell cases
cover home/contact/needle/report URLs, development assets, draft helpers,
optional JSON-LD, extra stylesheets, and adversarial metadata. Another 1,331
text/attribute combinations compare parsed output and decoded Datastar values.
The `website` test checks parsing and serialization without executing scripts.

Intentional byte differences are tested explicitly: present valueless attributes
use `=""`, and quotes in text nodes stay literal. Both parse identically to
Maud's output. Carriage returns use `&#13;` so the decoded value is preserved;
Maud emits raw carriage returns, which HTML normalizes to LF. NUL is rejected
instead of silently becoming U+FFFD. Static literals and dynamic values follow
the same contracts.

The SSE fixture applies the consumer's framing algorithm, models SSE `data:`
lines, and verifies exact fragment-byte reconstruction for empty lines, terminal
newlines in content, Unicode, carriage returns, and event-like text. It does not
exercise an HTTP server or the browser's Datastar plugin.

## Live browser check

The opt-in `browser` test serves renderer-generated HTML and SSE from a bounded
loopback HTTP fixture. A fresh headless Chromium profile loads the supplied
Datastar module and executes browser-only assertions. The test stops the browser
and server and removes the temporary profile afterward.

```console
nix develop .#browser --command env \
  RSTML_SSR_DATASTAR_JS=/absolute/path/to/datastar.js \
  cargo test --test browser -- --ignored --nocapture
```

The browser shell supplies Chromium on Linux. `RSTML_SSR_CHROMIUM` can select an
explicit executable. The test is ignored by ordinary Cargo/Nix checks because it
needs a browser and an external Datastar module. Neither the module nor an HTTP
integration is added to the renderer.

Verified on Linux x86_64 with Chromium 157.0.8103.38 and the website's
`assets/js/datastar-csp.js` at revision
`37328727faf4511e53c5baacee9804a30f2cbc9a`, SHA-256
`ae8736ac77d6a2e70654b4f75ea34b285599d726780fa0d9fdac40afc3f81aef`. The website
is read only during this check.

All 22 browser assertions pass: HTML/SVG inline script execution and applied
styles, SVG namespaces, MathML text and annotation integration points, decoded
expressions and form values, active and reset fetch indicators,
boolean/optional/empty attributes after a patch, Unicode and
multiline/carriage-return text, escaped markup, a second event registered on
patched content, a second SSE patch, and no JavaScript errors. The server also
verifies the form POST body and Datastar request header, a follow-up GET, and
exactly one request for each patch endpoint.

An isolated negative control inserted the payload through `TrustedHtml` instead
of ordinary text interpolation. The browser detected the resulting text/DOM
corruption and reported failure. The mutation was discarded; the original
template passes.

This Datastar build maps a boolean `true` attribute expression to an empty
attribute and `false` to omission. For `aria-busy`, use string values such as
`$fetching ? 'true' : 'false'`. The browser check verifies both the existing
boolean behavior and these explicit ARIA strings.

## Raw-content audit

- Institution mark: fixed SVG opening/closing tags and frozen polygon geometry.
  As in the source website, the favicon stylesheet is stripped before insertion.
  The retained geometry has no scripts, event handlers, or external references.
- Arrow: a complete static SVG with one path and no executable content.
- The renderer fixture now writes the SVG roots and arrow path directly through
  `html!`; only the frozen imported polygon geometry remains `TrustedHtml`.
- Inline script/style browser probes contain fixed source only. Their HTML and
  SVG parsing paths preserve comparison operators and ampersands. Dynamic
  script/style values require `TrustedHtml`; assembled HTML raw text rejects
  closing-tag and script-tokenizer sequences across interpolation boundaries.
  JSON-LD and split-sequence cases are covered by `tests/raw_content.rs`.
- JSON-LD: a complete constant script block with known JSON. Metadata strings
  remain escaped text/attributes and cannot enter this block. Dynamic JSON-LD
  requires an audited serialization step that prevents literal `</script>` (for
  example, escaping `<` as `\u003c` after JSON serialization), followed by the
  explicit `TrustedHtml` boundary. The fixture is not a general JSON-LD API.
- External script URLs: ordinary escaped attributes. URL/protocol validation
  remains a caller responsibility. Script elements have no inline content.
- CAPTCHA configuration and Datastar expressions: escaped attribute strings,
  never raw HTML. Their own expression/schema validation belongs to the caller.

Run `nix develop --command cargo test --test website` to reproduce these checks.
Runtime unit tests inject reservation failures at every growth point in their
rendering fixtures. The test-only hook forces capacity overflow in
`try_reserve_exact`, exercising reservation-error mapping and propagation
without actual allocator exhaustion. Run
`nix develop --command cargo test -p rstml-ssr --lib`.
