#[test]
fn e2e() {
    trycmd::TestCases::new().case("tests/fixtures/*.toml");
}
