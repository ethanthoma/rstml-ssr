#[test]
fn template_diagnostics() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/pass/*.rs");
    cases.compile_fail("tests/ui/*.rs");
}
