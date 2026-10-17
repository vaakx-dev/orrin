use std::path::{Path, PathBuf};

use orrin_core::{Hash, Result};

use super::fill::native;
use super::tasks::Task;

pub const NAME: &str = "in_file";

const DIR: &str = ".orrin/inputs";

pub fn path(task: &Task) -> String {
    let id = Hash::of(task.inputs.join("\n").as_bytes());
    format!("{DIR}/{}.rsp", &id.as_str()[..16])
}

pub fn write(dir: &Path, task: &Task) -> Result<PathBuf> {
    let file = dir.join(path(task));
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let lines: Vec<String> = task.inputs.iter().map(|input| native(input)).collect();
    std::fs::write(&file, lines.join("\n"))?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_the_inputs_one_per_line() {
        let dir = tempfile::tempdir().unwrap();
        let task = Task { inputs: vec!["a/x.obj".into(), "my dir/y.obj".into()], out: None, also: Vec::new() };
        let file = write(dir.path(), &task).unwrap();
        assert!(path(&task).starts_with(".orrin/inputs/"));
        let want = format!("{}\n{}", native("a/x.obj"), native("my dir/y.obj"));
        assert_eq!(std::fs::read_to_string(file).unwrap(), want);
    }
}
