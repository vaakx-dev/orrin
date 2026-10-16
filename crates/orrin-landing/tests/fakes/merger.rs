use orrin_core::service::{Merged, Merger};

pub struct LineMerger;

impl Merger for LineMerger {
    fn name(&self) -> &'static str {
        "lines"
    }

    fn handles(&self, path: &str) -> bool {
        path.ends_with(".txt")
    }

    fn merge(&self, _: &str, base: &[u8], ours: &[u8], theirs: &[u8]) -> Merged {
        let [base, ours, theirs] =
            [base, ours, theirs].map(|b| String::from_utf8_lossy(b).lines().map(String::from).collect::<Vec<_>>());
        if base.len() != ours.len() || base.len() != theirs.len() {
            return Merged::Conflict("line counts differ".into());
        }
        let mut out = String::new();
        for ((b, o), t) in base.iter().zip(&ours).zip(&theirs) {
            let line = match (o == b, t == b) {
                (true, _) => t,
                (_, true) => o,
                _ if o == t => o,
                _ => return Merged::Conflict("both sides changed the same line".into()),
            };
            out.push_str(line);
            out.push('\n');
        }
        Merged::Clean(out.into_bytes())
    }
}
