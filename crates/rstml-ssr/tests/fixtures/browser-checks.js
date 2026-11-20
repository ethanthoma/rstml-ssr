globalThis.browser_test_errors = [];
const record_browser_error = (error) => {
  if (globalThis.browser_test_errors.length < 8) {
    globalThis.browser_test_errors.push(String(error).slice(0, 256));
  }
};
window.addEventListener("error", (event) =>
  record_browser_error(event.message),
);
window.addEventListener("unhandledrejection", (event) =>
  record_browser_error(event.reason),
);

const checks = [];
const check = (condition, name) => {
  if (!condition) throw new Error(name);
  checks.push(name);
};
const wait_for = async (predicate, name) => {
  const deadline_ms = performance.now() + 5000;
  while (!predicate()) {
    if (performance.now() >= deadline_ms) throw new Error(`Timed out: ${name}`);
    await new Promise((resolve) => setTimeout(resolve, 10));
  }
};

let report;
try {
  await import("/datastar.js");
  check(
    document.getElementById("math-glyph").namespaceURI ===
      "http://www.w3.org/1998/Math/MathML" &&
      document.getElementById("math-text").namespaceURI ===
        "http://www.w3.org/1999/xhtml",
    "MathML text integration",
  );
  check(
    document.getElementById("math-html").namespaceURI ===
      "http://www.w3.org/1999/xhtml" &&
      document.getElementById("math-html").parentElement.localName ===
        "annotation-xml",
    "MathML HTML annotation integration",
  );
  check(
    document.getElementById("math-svg").namespaceURI ===
      "http://www.w3.org/2000/svg" &&
      document.getElementById("math-svg-html").namespaceURI ===
        "http://www.w3.org/1999/xhtml",
    "MathML SVG annotation integration",
  );
  check(
    globalThis.inline_script_checked === "A&B" &&
      globalThis.foreign_script_checked === "SVG&A",
    "inline script preserves operators",
  );
  check(
    document.getElementById("inline-path").namespaceURI ===
      "http://www.w3.org/2000/svg",
    "direct SVG namespace",
  );
  check(
    getComputedStyle(document.getElementById("inline-style")).color ===
      "rgb(12, 34, 56)" &&
      getComputedStyle(document.getElementById("foreign-style")).color ===
        "rgb(12, 34, 56)",
    "inline style applies",
  );
  const expected = await (await fetch("/expected")).json();
  const form = document.getElementById("patch-form");
  const send = document.getElementById("send");
  const note = document.getElementById("note");
  check(
    form.getAttribute("data-on:submit") === expected.submit_expression,
    "decoded submit expression",
  );
  check(note.value === expected.note, "decoded form value");
  check(!send.disabled, "initial signal state");
  check(document.getElementById("result").hidden, "initial hidden attribute");

  send.click();
  await wait_for(() => send.disabled, "fetch indicator becomes active");
  check(form.getAttribute("aria-busy") === "", "indicator updates aria-busy");
  check(
    document.getElementById("aria-status").getAttribute("aria-busy") === "true",
    "explicit ARIA true string",
  );
  await wait_for(
    () => document.getElementById("result").dataset.round === "1",
    "SSE patch",
  );
  await wait_for(() => !send.disabled, "fetch indicator resets");
  check(!form.hasAttribute("aria-busy"), "false signal removes attribute");
  check(
    document.getElementById("aria-status").getAttribute("aria-busy") ===
      "false",
    "explicit ARIA false string",
  );
  const result = document.getElementById("result");
  check(
    !result.hidden && !result.hasAttribute("hidden"),
    "false boolean omitted after patch",
  );
  check(
    !result.hasAttribute("data-optional"),
    "None removes previous attribute",
  );
  check(
    result.hasAttribute("data-empty") &&
      result.getAttribute("data-empty") === "",
    "empty attribute remains present",
  );
  check(result.title === expected.note, "patched attribute value");
  check(
    document.getElementById("payload").textContent === expected.text,
    "multiline Unicode and carriage returns",
  );
  check(
    result.querySelector("img") === null && globalThis.test_xss_fired !== true,
    "untrusted markup stays text",
  );

  document.getElementById("followup").click();
  await wait_for(
    () =>
      document.getElementById("confirmation").textContent ===
      "Second event worked",
    "event on patched content",
  );
  check(
    document.getElementById("confirmation").dataset.round === "2",
    "second SSE patch",
  );
  check(
    globalThis.browser_test_errors.length === 0,
    "no browser JavaScript errors",
  );
  report = { passed: true, checks, browser: navigator.userAgent };
  document.body.dataset.browserTest = "passed";
} catch (error) {
  report = {
    passed: false,
    checks,
    error: String(error),
    errors: globalThis.browser_test_errors,
    html: document.documentElement.outerHTML.slice(0, 2000),
  };
  document.body.dataset.browserTest = "failed";
}
await fetch("/report", {
  method: "POST",
  headers: { "Content-Type": "application/json" },
  body: JSON.stringify(report),
});
