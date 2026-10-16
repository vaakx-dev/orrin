use std::ffi::OsString;
use std::path::{Path, PathBuf};

use orrin_core::model::Tool;
use orrin_core::service::Services;

pub type Tools = Vec<(String, Option<Tool>)>;

pub async fn resolve(hub: &Services, names: &[String]) -> Tools {
    let mut tools = Vec::new();
    for name in names {
        let tool = hub.facts.tool(name).await.ok().flatten();
        tools.push((name.clone(), tool));
    }
    tools
}

pub async fn find(hub: &Services, name: &str) -> Option<PathBuf> {
    match hub.facts.tool(name).await {
        Ok(Some(tool)) => Some(PathBuf::from(tool.path)),
        _ => which(name),
    }
}

pub fn all_known(tools: &Tools) -> bool {
    tools.iter().all(|(_, tool)| tool.is_some())
}

pub fn search_path(tools: &Tools) -> Option<OsString> {
    let dirs =
        tools.iter().filter_map(|(_, t)| t.as_ref()).filter_map(|t| Path::new(&t.path).parent().map(Path::to_path_buf));
    let current = std::env::var_os("PATH").unwrap_or_default();
    std::env::join_paths(dirs.chain(std::env::split_paths(&current))).ok()
}

pub fn command_line(command: &str, tools: &Tools) -> String {
    let Some(program) = program(command) else { return command.to_string() };
    let known = tools.iter().find_map(|(name, tool)| tool.as_ref().filter(|_| name.eq_ignore_ascii_case(&program)));
    match known {
        Some(tool) => {
            let rest = command.trim_start().strip_prefix(program.as_str()).unwrap_or_default();
            format!("\"{}\"{rest}", tool.path)
        }
        None => command.to_string(),
    }
}

pub async fn learn(hub: &Services, command: &str, tools: &Tools) {
    let mut names: Vec<String> = tools.iter().filter(|(_, t)| t.is_none()).map(|(n, _)| n.clone()).collect();
    if let Some(program) = program(command) {
        names.push(program);
    }
    for name in names {
        if matches!(hub.facts.tool(&name).await, Ok(Some(_))) {
            continue;
        }
        if let Some(path) = which(&name) {
            let label =
                Path::new(&name).file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or(name.clone());
            let _ = hub.facts.learn(&label, &path.to_string_lossy(), None).await;
        }
    }
}

fn program(command: &str) -> Option<String> {
    let command = command.trim_start();
    let token = match command.strip_prefix('"') {
        Some(rest) => rest.split('"').next()?,
        None => command.split_whitespace().next()?,
    };
    (!token.is_empty()).then(|| token.to_string())
}

fn which(name: &str) -> Option<PathBuf> {
    let direct = Path::new(name);
    if direct.components().count() > 1 {
        return direct.is_file().then(|| direct.to_path_buf());
    }
    let exts = if direct.extension().is_some() { vec![String::new()] } else { extensions() };
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .flat_map(|dir| exts.iter().map(move |ext| dir.join(format!("{name}{ext}"))))
        .find(|candidate| candidate.is_file())
}

fn extensions() -> Vec<String> {
    if cfg!(windows) {
        let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into());
        pathext.split(';').filter(|e| !e.is_empty()).map(str::to_lowercase).collect()
    } else {
        vec![String::new()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_program_of_a_command() {
        assert_eq!(program("  cargo build --release").as_deref(), Some("cargo"));
        assert_eq!(
            program("\"C:/Program Files/Godot/godot.exe\" --export").as_deref(),
            Some("C:/Program Files/Godot/godot.exe")
        );
    }

    #[test]
    fn runs_a_needed_tool_by_its_known_path() {
        let path = "C:/Godot/godot.windows.opt.tools.64.exe".to_string();
        let tool = Tool { name: "godot".into(), path, version: None, source: orrin_core::model::ToolSource::Registry };
        let tools = vec![("godot".to_string(), Some(tool))];
        assert_eq!(
            command_line("godot --headless --quit", &tools),
            "\"C:/Godot/godot.windows.opt.tools.64.exe\" --headless --quit"
        );
        assert_eq!(command_line("cargo build", &tools), "cargo build");
    }
}
