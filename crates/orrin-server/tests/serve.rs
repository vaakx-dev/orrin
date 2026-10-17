mod daemon;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use daemon::Daemon;
use orrin_core::daemon::DaemonInfo;
use serde_json::json;

#[tokio::test]
async fn serves_typed_requests() {
    let daemon = Daemon::start().await;
    let home = daemon.home.clone();
    assert_eq!(DaemonInfo::read(&home).unwrap().port, daemon.port);
    let pong = daemon.call("ping", json!({})).await;
    assert_eq!(pong["ok"], true);
    assert!(pong["result"]["pc"].as_str().unwrap().starts_with("pc_"));

    let unknown = daemon.call("read", json!({ "project": "p_1", "path": "a.txt" })).await;
    assert_eq!(unknown["error"]["code"], "not_found");

    let bad = daemon.call("read", json!({ "project": "p_1", "path": "a.txt", "extra": 1 })).await;
    assert_eq!(bad["error"]["code"], "invalid");

    let missing = daemon.call("nope", json!({})).await;
    assert_eq!(missing["error"]["code"], "not_found");

    let events = format!("ws://127.0.0.1:{}/events?key={}", daemon.port, home.local_key().unwrap());
    let (socket, response) = tokio_tungstenite::connect_async(events).await.unwrap();
    assert_eq!(response.status(), 101);
    drop(socket);

    daemon.stop().await;
    assert!(DaemonInfo::read(&home).is_err());
}

#[tokio::test]
async fn objects_get_returns_whole_big_files() {
    let daemon = Daemon::start().await;
    let folder = tempfile::tempdir().unwrap();
    let mut seed = 7u32;
    let bytes: Vec<u8> = (0..3 << 20)
        .map(|_| {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 24) as u8
        })
        .collect();
    std::fs::write(folder.path().join("shot.png"), &bytes).unwrap();
    let opened = daemon.call("projects.import", json!({ "path": folder.path() })).await;
    let project = &opened["result"]["project"]["id"];
    let read = daemon.call("read", json!({ "project": project, "path": "shot.png" })).await;
    let hash = &read["result"]["hash"];

    let object = daemon.call("objects.get", json!({ "hash": hash })).await;
    let data = STANDARD.decode(object["result"]["data"].as_str().unwrap()).unwrap();
    assert!(data == bytes);
    daemon.stop().await;
}

#[tokio::test]
async fn serves_compiler_cache_entries_over_webdav() {
    let daemon = Daemon::start().await;
    let http = reqwest::Client::new();
    let token = format!("Bearer {}", daemon.home.local_key().unwrap());
    let url = daemon.url("/cache/a/b/c/abc123");
    let send = |method: &str, url: &str| {
        http.request(reqwest::Method::from_bytes(method.as_bytes()).unwrap(), url).header("authorization", &token)
    };

    assert_eq!(http.get(&url).send().await.unwrap().status(), 401);
    assert_eq!(send("GET", &url).send().await.unwrap().status(), 404);
    assert_eq!(send("PROPFIND", &daemon.url("/cache/a/b/c/")).send().await.unwrap().status(), 207);
    assert_eq!(send("MKCOL", &daemon.url("/cache/a/")).send().await.unwrap().status(), 201);
    let entry = vec![7u8; 3 << 20];
    assert_eq!(send("PUT", &url).body(entry.clone()).send().await.unwrap().status(), 201);
    let got = send("GET", &url).send().await.unwrap();
    assert_eq!(got.status(), 200);
    assert!(got.bytes().await.unwrap() == entry);
    daemon.stop().await;
}

#[tokio::test]
async fn local_callers_need_the_key() {
    let daemon = Daemon::start().await;
    let http = reqwest::Client::new();
    let bare = http.post(daemon.url("/api/ping")).json(&json!({})).send().await.unwrap();
    assert_eq!(bare.status(), 401);
    let wrong = http.post(daemon.url("/api/ping")).header("x-orrin-local", "nope").json(&json!({}));
    assert_eq!(wrong.send().await.unwrap().status(), 401);
    assert!(tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{}/events", daemon.port)).await.is_err());
    assert_eq!(daemon.call("ping", json!({})).await["ok"], true);
    daemon.stop().await;
}
