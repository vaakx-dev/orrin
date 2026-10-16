mod daemon;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use daemon::Daemon;
use serde_json::{Value, json};

fn with_fingerprint(code: &str, fingerprint: &str) -> String {
    let body = URL_SAFE_NO_PAD.decode(code.strip_prefix("orrin-").unwrap()).unwrap();
    let mut fields: Value = serde_json::from_slice(&body).unwrap();
    fields["fingerprint"] = json!(fingerprint);
    format!("orrin-{}", URL_SAFE_NO_PAD.encode(serde_json::to_vec(&fields).unwrap()))
}

async fn pc(daemon: &Daemon) -> Value {
    daemon.call("ping", json!({})).await["result"]["pc"].clone()
}

#[tokio::test(flavor = "multi_thread")]
async fn peers_pair_and_call_each_other_over_https() {
    let a = Daemon::start().await;
    let b = Daemon::start().await;
    let code = a.call("pcs.pair", json!({})).await["result"]["code"].as_str().unwrap().to_string();

    let forged = b.call("pcs.redeem", json!({ "code": with_fingerprint(&code, &"0".repeat(64)), "address": "" })).await;
    assert_eq!(forged["ok"], false, "{forged}");

    let paired = b.call("pcs.redeem", json!({ "code": code, "address": "" })).await;
    assert_eq!(paired["ok"], true, "{paired}");
    let (a_pc, b_pc) = (pc(&a).await, pc(&b).await);
    assert_eq!(paired["result"]["id"], a_pc);

    let from_a = a.call("facts", json!({ "pc": b_pc })).await;
    assert_eq!(from_a["result"]["pc"], b_pc, "{from_a}");
    let from_b = b.call("facts", json!({ "pc": a_pc })).await;
    assert_eq!(from_b["result"]["pc"], a_pc, "{from_b}");

    a.stop().await;
    b.stop().await;
}
