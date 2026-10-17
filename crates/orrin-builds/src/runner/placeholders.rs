use std::path::Path;

use crate::process;

pub struct Values<'a> {
    pub dir: &'a Path,
    pub screenshots: &'a Path,
    pub job: &'a str,
    pub target: &'a str,
    pub args: &'a [String],
}

pub struct Expanded {
    pub command: String,
    pub took_args: bool,
}

pub fn expand(command: &str, values: &Values) -> Expanded {
    let names = [
        ("{screenshots}", values.screenshots.to_string_lossy().into_owned()),
        ("{dir}", values.dir.to_string_lossy().into_owned()),
        ("{job}", values.job.to_string()),
        ("{target}", values.target.to_string()),
        ("{args}", process::join_args(values.args)),
    ];
    let mut out = String::with_capacity(command.len());
    let mut rest = command;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let tail = &rest[start..];
        match names.iter().find(|(token, _)| tail.starts_with(token)) {
            Some((token, value)) => {
                out.push_str(value);
                rest = &tail[token.len()..];
            }
            None => {
                out.push('{');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    Expanded { command: out, took_args: command.contains("{args}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_placeholders_the_same_on_every_os() {
        let dir = Path::new("runs").join("j1");
        let shots = dir.join("shots");
        let args = vec!["--release".to_string()];
        let values = Values { dir: &dir, screenshots: &shots, job: "j1", target: "app", args: &args };
        let expanded = expand("tool {target} -o {dir} -s {screenshots} {job} {args} {keep}", &values);
        let want = format!("tool app -o {} -s {} j1 --release {{keep}}", dir.display(), shots.display());
        assert_eq!(expanded.command, want);
        assert!(expanded.took_args);
        assert!(!expand("make", &values).took_args);
    }
}
