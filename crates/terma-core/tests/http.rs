use terma_core::error::TermaError;
use terma_core::http::{FakeHttp, Http, UreqHttp};

#[test]
fn ureq_http_rejects_plain_http() {
    let err = UreqHttp::new().get_string("http://aur.archlinux.org/").unwrap_err();
    assert!(matches!(err, TermaError::Network(_)));
}

#[test]
fn fake_http_serves_registered_urls() {
    let http = FakeHttp::new().on("https://example.org/a", "hallo");
    assert_eq!(http.get_string("https://example.org/a").unwrap(), "hallo");
    assert!(matches!(http.get_string("https://example.org/b"), Err(TermaError::Network(_))));
}
