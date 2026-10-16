use std::collections::HashSet;

use orrin_core::config::HistoryConfig;
use orrin_core::{ChangeId, Millis};

const DAY: Millis = 24 * 60 * 60 * 1000;
const WEEK: Millis = 7 * DAY;

pub struct Dated {
    pub id: ChangeId,
    pub at: Millis,
    pub pinned: bool,
}

fn one_per_bucket(newest_first: &[Dated], span: Millis, buckets: u32, keep: &mut [bool]) {
    let mut seen = HashSet::new();
    for (i, change) in newest_first.iter().enumerate() {
        if seen.len() as u32 >= buckets {
            return;
        }
        if seen.insert(change.at / span) {
            keep[i] = true;
        }
    }
}

pub fn doomed(newest_first: &[Dated], history: &HistoryConfig) -> Vec<ChangeId> {
    let latest = (history.latest as usize).max(1);
    let mut keep: Vec<bool> = newest_first.iter().enumerate().map(|(i, c)| i < latest || c.pinned).collect();
    one_per_bucket(newest_first, DAY, history.daily, &mut keep);
    one_per_bucket(newest_first, WEEK, history.weekly, &mut keep);
    newest_first.iter().zip(keep).filter(|(_, kept)| !kept).map(|(c, _)| c.id.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dated(id: &str, at: Millis, pinned: bool) -> Dated {
        Dated { id: ChangeId::from(id), at, pinned }
    }

    #[test]
    fn keeps_latest_daily_weekly_and_pinned() {
        let history = HistoryConfig { latest: 2, daily: 2, weekly: 2 };
        let changes = vec![
            dated("today-3", 10 * WEEK + 3, false),
            dated("today-2", 10 * WEEK + 2, false),
            dated("today-1", 10 * WEEK + 1, false),
            dated("yesterday-2", 10 * WEEK - DAY + 2, false),
            dated("yesterday-1", 10 * WEEK - DAY + 1, false),
            dated("old-week", 8 * WEEK, false),
            dated("older-week", 5 * WEEK, false),
            dated("ancient-pinned", WEEK, true),
        ];
        let gone: Vec<String> = doomed(&changes, &history).into_iter().map(|id| id.0).collect();
        assert_eq!(gone, ["today-1", "yesterday-1", "old-week", "older-week"]);
    }
}
