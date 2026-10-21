#[test]
fn e2e() {
    let cases = trycmd::TestCases::new();
    cases.case("tests/fixtures/*.toml");
    #[cfg(windows)]
    cases.case("tests/fixtures/windows/*.toml");
    #[cfg(target_os = "linux")]
    cases.case("tests/fixtures/linux/*.toml");
}
