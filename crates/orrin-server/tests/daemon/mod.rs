use orrin_core::Home;
use orrin_core::daemon::LOCAL_KEY_HEADER;
use orrin_server::Server;
use serde_json::Value;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

pub struct Daemon {
    pub home: Home,
    pub port: u16,
    stop: oneshot::Sender<()>,
    task: JoinHandle<orrin_core::Result<()>>,
    _dir: tempfile::TempDir,
}

impl Daemon {
    pub async fn start() -> Self {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("orrin.toml"), "[server]\nbind = \"127.0.0.1\"\n").unwrap();
        let home = Home::at(dir.path()).unwrap();
        let server = Server::bind(home.clone(), Some(0)).await.unwrap();
        let port = server.port();
        let (stop, stopped) = oneshot::channel::<()>();
        let task = tokio::spawn(server.run(async move {
            let _ = stopped.await;
        }));
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        Self { home, port, stop, task, _dir: dir }
    }

    pub fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    pub async fn call(&self, name: &str, body: Value) -> Value {
        let request = reqwest::Client::new().post(self.url(&format!("/api/{name}")));
        let request = request.header(LOCAL_KEY_HEADER, self.home.local_key().unwrap()).json(&body);
        request.send().await.unwrap().json().await.unwrap()
    }

    pub async fn stop(self) {
        self.stop.send(()).unwrap();
        self.task.await.unwrap().unwrap();
    }
}
