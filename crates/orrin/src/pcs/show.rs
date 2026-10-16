use orrin_core::api::PcsPairOutput;
use orrin_core::model::Pc;
use orrin_core::now_ms;

use crate::table::{Table, ago};

pub fn pcs(pcs: &[Pc]) -> String {
    let mut table = Table::new(&["NAME", "ID", "ADDRESS", "SEEN", "BUILDS"]);
    for pc in pcs {
        let name = if pc.me { format!("{} (this PC)", pc.name) } else { pc.name.clone() };
        let seen = match (pc.me, pc.online, pc.last_seen) {
            (true, _, _) | (_, true, _) => "online".to_string(),
            (_, _, Some(at)) => ago(at),
            _ => "never".to_string(),
        };
        let builds = if pc.settings.builds { "on" } else { "off" };
        table.row(vec![name, pc.id.to_string(), pc.address.clone().unwrap_or_default(), seen, builds.into()]);
    }
    table.render("No PCs.")
}

pub fn pair(out: &PcsPairOutput) -> String {
    let minutes = out.expires_at.saturating_sub(now_ms()).div_ceil(60_000);
    format!("On the other PC run this within {minutes} min:\n  orrin redeem {}", out.code)
}

pub fn redeemed(pc: &Pc) -> String {
    format!("Paired with {} ({})", pc.name, pc.id)
}

pub struct Status {
    pub name: String,
    pub version: String,
    pub port: u16,
    pub projects: usize,
    pub open_views: usize,
    pub active_jobs: usize,
    pub peers: usize,
    pub peers_online: usize,
    pub here: Option<String>,
}

pub fn status(s: &Status) -> String {
    let mut lines = vec![
        format!("orrin {} on {}, port {}", s.version, s.name, s.port),
        format!("projects  {}", s.projects),
        format!("views     {} open", s.open_views),
        format!("jobs      {} active", s.active_jobs),
        format!("peers     {} paired, {} online", s.peers, s.peers_online),
    ];
    lines.extend(s.here.as_ref().map(|here| format!("here      {here}")));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reads_as_a_short_summary() {
        let s = Status {
            name: "desk".into(),
            version: "0.1.0".into(),
            port: 4520,
            projects: 2,
            open_views: 3,
            active_jobs: 1,
            peers: 1,
            peers_online: 0,
            here: None,
        };
        assert_eq!(
            status(&s),
            "orrin 0.1.0 on desk, port 4520\nprojects  2\nviews     3 open\njobs      1 active\npeers     1 paired, 0 online"
        );
    }
}
