use std::sync::OnceLock;

use tree_sitter::{Language, Query};

use super::queries;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    Rust,
    TypeScript,
    Tsx,
    JavaScript,
    Python,
    C,
    Cpp,
    GdScript,
}

const COUNT: usize = 8;

static QUERIES: [OnceLock<Option<Query>>; COUNT] = [const { OnceLock::new() }; COUNT];

impl Lang {
    pub fn from_extension(ext: &str) -> Option<Self> {
        Some(match ext {
            "rs" => Lang::Rust,
            "ts" | "mts" | "cts" => Lang::TypeScript,
            "tsx" => Lang::Tsx,
            "js" | "mjs" | "cjs" | "jsx" => Lang::JavaScript,
            "py" | "pyi" => Lang::Python,
            "c" | "h" => Lang::C,
            "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" | "inl" => Lang::Cpp,
            "gd" => Lang::GdScript,
            _ => return None,
        })
    }

    pub fn key(self) -> &'static str {
        match self {
            Lang::Rust => "rust",
            Lang::TypeScript => "typescript",
            Lang::Tsx => "tsx",
            Lang::JavaScript => "javascript",
            Lang::Python => "python",
            Lang::C => "c",
            Lang::Cpp => "cpp",
            Lang::GdScript => "gdscript",
        }
    }

    pub fn language(self) -> Language {
        match self {
            Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
            Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX.into(),
            Lang::JavaScript => tree_sitter_javascript::LANGUAGE.into(),
            Lang::Python => tree_sitter_python::LANGUAGE.into(),
            Lang::C => tree_sitter_c::LANGUAGE.into(),
            Lang::Cpp => tree_sitter_cpp::LANGUAGE.into(),
            Lang::GdScript => tree_sitter_gdscript::LANGUAGE.into(),
        }
    }

    pub fn query(self) -> Option<&'static Query> {
        QUERIES[self as usize].get_or_init(|| self.compile()).as_ref()
    }

    fn compile(self) -> Option<Query> {
        let source = match self {
            Lang::Rust => queries::RUST.to_string(),
            Lang::TypeScript | Lang::Tsx => format!("{}{}", queries::SCRIPT, queries::TYPESCRIPT),
            Lang::JavaScript => format!("{}{}", queries::SCRIPT, queries::JAVASCRIPT),
            Lang::Python => queries::PYTHON.to_string(),
            Lang::C => queries::C.to_string(),
            Lang::Cpp => format!("{}{}", queries::C, queries::CPP),
            Lang::GdScript => queries::GDSCRIPT.to_string(),
        };
        Query::new(&self.language(), &source)
            .inspect_err(|err| tracing::warn!("symbol query for {} failed: {err}", self.key()))
            .ok()
    }
}
