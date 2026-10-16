mod daemon;

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
