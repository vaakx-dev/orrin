use orrin_core::Home;
use orrin_core::daemon::DaemonInfo;
use orrin_server::Server;
use serde_json::{Value, json};

async fn post(port: u16, name: &str, body: Value) -> Value {
    let url = format!("http://127.0.0.1:{port}/api/{name}");
    reqwest::Client::new().post(url).json(&body).send().await.unwrap().json().await.unwrap()
}

#[tokio::test]
async fn serves_typed_requests() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("orrin.toml"), "[server]\nbind = \"127.0.0.1\"\n").unwrap();
    let home = Home::at(dir.path()).unwrap();
    let server = Server::bind(home.clone(), Some(0)).await.unwrap();
    let port = server.port();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let task = tokio::spawn(server.run(async move {
        let _ = stopped.await;
    }));
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    assert_eq!(DaemonInfo::read(&home).unwrap().port, port);
    let pong = post(port, "ping", json!({})).await;
    assert_eq!(pong["ok"], true);
    assert!(pong["result"]["pc"].as_str().unwrap().starts_with("pc_"));

    let unknown = post(port, "read", json!({ "project": "p_1", "path": "a.txt" })).await;
    assert_eq!(unknown["error"]["code"], "not_found");

    let bad = post(port, "read", json!({ "project": "p_1", "path": "a.txt", "extra": 1 })).await;
    assert_eq!(bad["error"]["code"], "invalid");

    let missing = post(port, "nope", json!({})).await;
    assert_eq!(missing["error"]["code"], "not_found");

    let (socket, response) = tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/events")).await.unwrap();
    assert_eq!(response.status(), 101);
    drop(socket);

    stop.send(()).unwrap();
    task.await.unwrap().unwrap();
    assert!(DaemonInfo::read(&home).is_err());
}
