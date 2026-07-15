use serde_json::Value;

#[test]
fn opener_scope_allows_only_the_loopback_download_helper() {
    let capability: Value = serde_json::from_str(include_str!("../capabilities/default.json"))
        .expect("default capability must be valid JSON");
    let permissions = capability["permissions"]
        .as_array()
        .expect("permissions must be an array");
    let opener = permissions
        .iter()
        .find(|permission| permission["identifier"] == "opener:allow-open-url")
        .expect("open URL permission must exist");
    let urls = opener["allow"]
        .as_array()
        .expect("open URL permission must have a scope")
        .iter()
        .filter_map(|entry| entry["url"].as_str())
        .collect::<Vec<_>>();

    assert!(urls.contains(&"https://*"));
    assert!(urls.contains(&"http://127.0.0.1:*/download"));
    assert!(!urls.contains(&"http://*"));
}
