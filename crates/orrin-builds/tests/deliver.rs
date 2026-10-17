mod fakes;

use std::time::Duration;

use orrin_core::ChangeId;
use orrin_core::api::Event;
use orrin_core::model::{Delivery, DeliveryState};

use fakes::world;

#[cfg(windows)]
const WRITE: &str = "echo built> out.txt";
#[cfg(not(windows))]
const WRITE: &str = "echo built > out.txt";

#[tokio::test(flavor = "multi_thread")]
async fn moving_main_builds_and_delivers_and_waits_for_busy_files() {
    let manifest = format!("[targets.app]\ncommand = '{WRITE}'\noutputs = [\"*.txt\"]\ndeliver = true\n");
    let w = world(&[("orrin.toml", &manifest)]);
    *w.views.busy.lock().unwrap() = vec!["out.txt".into()];
    let mut events = w.setup.events.subscribe();
    w.setup.events.emit(Event::MainMoved { project: w.project.clone(), main: w.main(), change: ChangeId::new() });

    let mut seen: Vec<Delivery> = Vec::new();
    let settled = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            if let Event::Delivery { delivery } = events.recv().await.unwrap().event {
                let state = delivery.state;
                seen.push(delivery);
                if matches!(state, DeliveryState::Done | DeliveryState::Failed) {
                    return state;
                }
            }
        }
    })
    .await;
    assert_eq!(settled, Ok(DeliveryState::Done), "{seen:?}");
    let waiting = seen.iter().find(|d| d.state == DeliveryState::Waiting).expect("a waiting event");
    assert_eq!(waiting.waiting, ["out.txt"]);
    assert_eq!(*w.views.delivered.lock().unwrap(), [["out.txt"], ["out.txt"]]);
    let last = w.builds.delivery(&w.project).await.unwrap();
    assert_eq!((last.state, last.copied, last.waiting.len()), (DeliveryState::Done, 1, 0));
}
