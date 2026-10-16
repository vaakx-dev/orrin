use orrin_core::{Millis, now_ms};

pub struct Table {
    head: Vec<&'static str>,
    rows: Vec<Vec<String>>,
}

impl Table {
    pub fn new(head: &[&'static str]) -> Self {
        Self { head: head.to_vec(), rows: Vec::new() }
    }

    pub fn row(&mut self, cells: Vec<String>) {
        self.rows.push(cells);
    }

    pub fn render(&self, empty: &str) -> String {
        if self.rows.is_empty() {
            return empty.to_string();
        }
        let head: Vec<String> = self.head.iter().map(|h| h.to_string()).collect();
        let lines: Vec<&Vec<String>> = std::iter::once(&head).chain(&self.rows).collect();
        let widths: Vec<usize> = (0..head.len())
            .map(|i| lines.iter().map(|row| row.get(i).map_or(0, |c| c.chars().count())).max().unwrap_or(0))
            .collect();
        lines.iter().map(|row| line(row, &widths)).collect::<Vec<_>>().join("\n")
    }
}

fn line(row: &[String], widths: &[usize]) -> String {
    let cells: Vec<String> = row.iter().zip(widths).map(|(cell, width)| format!("{cell:<width$}")).collect();
    cells.join("  ").trim_end().to_string()
}

pub fn short(text: &str) -> String {
    text.chars().take(10).collect()
}

pub fn ago(at: Millis) -> String {
    let secs = now_ms().saturating_sub(at) / 1000;
    match secs {
        0..60 => format!("{secs}s ago"),
        60..3600 => format!("{}m ago", secs / 60),
        3600..86400 => format!("{}h ago", secs / 3600),
        _ => format!("{}d ago", secs / 86400),
    }
}

pub fn size(bytes: u64) -> String {
    match bytes {
        0..1024 => format!("{bytes} B"),
        1024..1048576 => format!("{:.1} KB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MB", bytes as f64 / 1048576.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn columns_line_up() {
        let mut table = Table::new(&["NAME", "ID"]);
        table.row(vec!["game".into(), "p_1".into()]);
        table.row(vec!["website".into(), "p_2".into()]);
        assert_eq!(table.render(""), "NAME     ID\ngame     p_1\nwebsite  p_2");
        assert_eq!(Table::new(&["X"]).render("none"), "none");
    }

    #[test]
    fn sizes_read_well() {
        assert_eq!(size(12), "12 B");
        assert_eq!(size(2048), "2.0 KB");
        assert_eq!(size(3 * 1048576), "3.0 MB");
    }
}
