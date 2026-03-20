#![no_main]

use libfuzzer_sys::fuzz_target;

use harrow_core::path::PathMatch;
use harrow_core::request::Request;
use harrow_core::state::TypeMap;
use std::sync::Arc;

fuzz_target!(|data: &[u8]| {
    let query_string = String::from_utf8_lossy(data);

    // Build an HTTP request with the fuzzed query string
    let uri = format!("/?{query_string}");
    let inner = match http::Request::builder()
        .method("GET")
        .uri(&uri)
        .body(harrow_core::request::full_body(http_body_util::Full::new(
            bytes::Bytes::new(),
        ))) {
        Ok(req) => req,
        // Invalid URI is fine — just skip this input
        Err(_) => return,
    };

    let req = Request::new(inner, PathMatch::default(), Arc::new(TypeMap::new()), None);

    // query_pairs() must not panic and must respect the cap
    let pairs = req.query_pairs();
    assert!(pairs.len() <= 100);

    // Agreement: for each key in query_pairs(), query_param(key) must return
    // the same value (query_param finds the *first* occurrence, while
    // query_pairs collects into a HashMap where last-write-wins; they agree
    // only when keys are unique — so we check containment instead).
    for (key, _value) in &pairs {
        let param = req.query_param(key);
        assert!(
            param.is_some(),
            "query_param({:?}) returned None but key exists in query_pairs()",
            key,
        );
    }

    // A key that doesn't exist in pairs should return None
    // (unless the fuzz data happens to contain it)
    if !pairs.contains_key("__nonexistent__") {
        assert!(
            req.query_param("__nonexistent__").is_none(),
            "query_param for missing key should return None",
        );
    }
});
