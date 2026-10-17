use std::collections::HashSet;

use orrin_core::model::{PREVIOUS, Rule, Step};
use orrin_core::{Error, Result};

use super::fill::{Values, fill};
use crate::paths;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Task {
    pub inputs: Vec<String>,
    pub out: Option<String>,
    pub also: Vec<String>,
}

pub fn expand(step: &Step, rule: &Rule, files: &[String], previous: &[String], vars: &Values) -> Result<Vec<Task>> {
    let tasks = match step.files.is_empty() {
        false => {
            let globs = paths::globs(&step.files)?;
            let matched = files.iter().filter(|path| globs.is_match(path.as_str()));
            matched.map(|path| task(rule, vec![path.clone()], output(&rule.output, path, vars), vars)).collect()
        }
        true => {
            let inputs = gather(&step.input, files, previous)?;
            let out = match &step.out {
                Some(out) => Some(fill(out, vars)),
                None => inputs.first().and_then(|first| output(&rule.output, first, vars)),
            };
            vec![task(rule, inputs, out, vars)]
        }
    };
    let mut seen = HashSet::new();
    for out in tasks.iter().filter_map(|task| task.out.as_deref()) {
        if !seen.insert(out) {
            return Err(Error::invalid(format!("two steps of rule {} write {out}", step.rule)));
        }
    }
    Ok(tasks)
}

pub fn names(path: &str) -> Values {
    let (dir, file) = path.rsplit_once('/').map_or(("", path), |(dir, file)| (dir, file));
    let (name, ext) = file.rsplit_once('.').filter(|(name, _)| !name.is_empty()).unwrap_or((file, ""));
    let stem = if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") };
    [("path", stem), ("name", name.to_string()), ("ext", ext.to_string())]
        .map(|(key, value)| (key.to_string(), value))
        .into()
}

fn task(rule: &Rule, inputs: Vec<String>, out: Option<String>, vars: &Values) -> Task {
    let mut values = vars.clone();
    if let Some(first) = inputs.first() {
        values.extend(names(first));
    }
    if let Some(out) = &out {
        values.extend(names(out).into_iter().map(|(name, value)| (format!("out.{name}"), value)));
        values.insert("out".into(), out.clone());
    }
    let also = rule.outputs.iter().map(|template| fill(template, &values)).collect();
    Task { inputs, out, also }
}

fn output(template: &str, input: &str, vars: &Values) -> Option<String> {
    if template.is_empty() {
        return None;
    }
    let mut values = vars.clone();
    values.extend(names(input));
    Some(fill(template, &values))
}

fn gather(patterns: &[String], files: &[String], previous: &[String]) -> Result<Vec<String>> {
    let mut inputs = Vec::new();
    for pattern in patterns {
        match pattern.as_str() {
            PREVIOUS => inputs.extend(previous.iter().cloned()),
            glob => {
                let globs = paths::globs(&[glob.to_string()])?;
                inputs.extend(files.iter().filter(|path| globs.is_match(path.as_str())).cloned());
            }
        }
    }
    Ok(inputs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(text: &str) -> Step {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn expands_one_task_per_file_and_links_the_previous_outputs() {
        let files: Vec<String> = ["game/a.cpp", "game/x/b.cpp", "game/a.h", "README"].map(String::from).into();
        let cxx = Rule { output: ".build/{path}.obj".into(), ..Default::default() };
        let compiled =
            expand(&step("rule = 'cxx'\nfiles = 'game/**/*.cpp'"), &cxx, &files, &[], &Values::new()).unwrap();
        let outs: Vec<_> = compiled.iter().map(|t| t.out.clone().unwrap()).collect();
        assert_eq!(outs, [".build/game/a.obj", ".build/game/x/b.obj"]);
        let link = step("rule = 'dll'\nin = '@previous'\nout = 'bin/{lib}.dll'");
        let vars: Values = [("lib".to_string(), "x".to_string())].into();
        let dll = Rule { outputs: vec!["{out.path}.lib".into(), "{out.path}.pdb".into()], ..Default::default() };
        let linked = expand(&link, &dll, &files, &outs, &vars).unwrap();
        let also = vec!["bin/x.lib".to_string(), "bin/x.pdb".to_string()];
        assert_eq!(linked, [Task { inputs: outs, out: Some("bin/x.dll".into()), also }]);
        let clash = Rule { output: "{name}.obj".into(), ..Default::default() };
        let two: Vec<String> = ["a/x.cpp", "b/x.cpp"].map(String::from).into();
        assert!(expand(&step("rule = 'c'\nfiles = '**/*.cpp'"), &clash, &two, &[], &Values::new()).is_err());
    }

    #[test]
    fn names_split_path_name_and_extension() {
        let values = names("src/game/main.cpp");
        assert_eq!((&values["path"][..], &values["name"][..], &values["ext"][..]), ("src/game/main", "main", "cpp"));
        assert_eq!(names(".gitignore")["name"], ".gitignore");
    }
}
