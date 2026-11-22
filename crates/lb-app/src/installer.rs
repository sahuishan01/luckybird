//! Installs tree-sitter grammars (built from source) and language servers
//! (through each ecosystem's own installer). Everything runs on a background
//! thread and reports progress as one-line status messages.
//!
//! Only languages from the embedded grammar registry are accepted and every
//! command line comes from the tables below, so user input never reaches a
//! shell.

use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use tokio::sync::mpsc::UnboundedSender;

use crate::compile::compile_grammar;

/// Grammar repos that are not `tree-sitter/tree-sitter-<lang>`:
/// `(language, "owner/repo", subdirectory containing the grammar)`.
const GRAMMAR_REPOS: &[(&str, &str, &str)] = &[
    ("cpp", "tree-sitter/tree-sitter-cpp", ""),
    ("csharp", "tree-sitter/tree-sitter-c-sharp", ""),
    ("tsx", "tree-sitter/tree-sitter-typescript", "tsx"),
    ("typescript", "tree-sitter/tree-sitter-typescript", "typescript"),
    ("dockerfile", "camdencheek/tree-sitter-dockerfile", ""),
    ("markdown", "MDeiml/tree-sitter-markdown", "tree-sitter-markdown"),
    ("elixir", "elixir-lang/tree-sitter-elixir", ""),
    ("erlang", "WhatsApp/tree-sitter-erlang", ""),
    ("haskell", "tree-sitter/tree-sitter-haskell", ""),
    ("kotlin", "fwcd/tree-sitter-kotlin", ""),
    ("ocaml", "tree-sitter/tree-sitter-ocaml", "grammars/ocaml"),
    ("scala", "tree-sitter/tree-sitter-scala", ""),
    ("sql", "derekstride/tree-sitter-sql", ""),
    ("swift", "alex-pinkus/tree-sitter-swift", ""),
    ("zig", "maxxnino/tree-sitter-zig", ""),
    ("fish", "ram02z/tree-sitter-fish", ""),
];

type Step = &'static [&'static str];

/// Language-server install commands, run in order.
fn server_steps(lang: &str) -> Option<&'static [Step]> {
    let steps: &'static [Step] = match lang {
        "rust" => &[&["rustup", "component", "add", "rust-analyzer"]],
        "python" => &[&["npm", "install", "-g", "pyright"]],
        "javascript" | "typescript" | "tsx" => {
            &[&["npm", "install", "-g", "typescript-language-server", "typescript"]]
        }
        "go" => &[&["go", "install", "golang.org/x/tools/gopls@latest"]],
        "bash" => &[&["npm", "install", "-g", "bash-language-server"]],
        "html" | "css" | "json" => &[&["npm", "install", "-g", "vscode-langservers-extracted"]],
        "yaml" => &[&["npm", "install", "-g", "yaml-language-server"]],
        "toml" => &[&["cargo", "install", "--locked", "--features", "lsp", "taplo-cli"]],
        _ => return None,
    };
    Some(steps)
}

/// Replace `dest` with `src`. A library the running editor has loaded cannot be
/// overwritten on Windows, but it can be renamed out of the way.
fn swap_in(src: &std::path::Path, dest: &std::path::Path) -> Result<(), String> {
    if dest.exists() && std::fs::remove_file(dest).is_err() {
        let old = dest.with_extension("old");
        let _ = std::fs::remove_file(&old);
        std::fs::rename(dest, &old)
            .map_err(|e| format!("cannot replace the loaded grammar: {e}"))?;
    }
    std::fs::copy(src, dest).map(|_| ()).map_err(|e| e.to_string())
}

/// Only languages from the embedded grammar registry may reach a command line.
fn is_known(lang: &str) -> bool {
    lb_syntax::registry::load().contains_key(lang)
}

fn grammar_source(lang: &str) -> (String, &'static str) {
    GRAMMAR_REPOS
        .iter()
        .find(|(l, _, _)| *l == lang)
        .map(|(_, repo, sub)| ((*repo).to_owned(), *sub))
        .unwrap_or_else(|| (format!("tree-sitter/tree-sitter-{lang}"), ""))
}

#[derive(Clone)]
pub struct Installer {
    tx: UnboundedSender<String>,
    grammar_dir: PathBuf,
    query_dir: PathBuf,
}

impl Installer {
    pub fn new(tx: UnboundedSender<String>, grammar_dir: PathBuf, query_dir: PathBuf) -> Self {
        Self { tx, grammar_dir, query_dir }
    }

    pub fn install_grammars(&self, langs: Vec<String>) {
        let this = self.clone();
        std::thread::spawn(move || {
            this.run_all("grammar", &langs, |lang| this.install_grammar(lang));
        });
    }

    pub fn install_servers(&self, langs: Vec<String>) {
        let this = self.clone();
        std::thread::spawn(move || {
            this.run_all("language server", &langs, |lang| this.install_server(lang));
        });
    }

    fn say(&self, msg: String) {
        tracing::info!(message = %msg, "installer");
        let _ = self.tx.send(msg);
    }

    fn run_all(&self, kind: &str, langs: &[String], one: impl Fn(&str) -> Result<(), String>) {
        let mut done = Vec::new();
        let mut failed = Vec::new();
        for lang in langs {
            if !is_known(lang) {
                failed.push(format!("{lang}: unknown language"));
                continue;
            }
            self.say(format!("Installing {kind} for {lang}..."));
            match one(lang) {
                Ok(()) => done.push(lang.as_str()),
                Err(e) => {
                    tracing::warn!(lang, error = %e, "install failed");
                    failed.push(format!("{lang}: {e}"));
                }
            }
        }
        let mut msg = String::new();
        if !done.is_empty() {
            msg.push_str(&format!("Installed {kind}: {}. Reopen files to use it.", done.join(", ")));
        }
        if !failed.is_empty() {
            if !msg.is_empty() {
                msg.push_str("  ");
            }
            msg.push_str(&format!("Failed: {}", failed.join("; ")));
        }
        self.say(msg);
    }

    fn install_grammar(&self, lang: &str) -> Result<(), String> {
        let (repo, subdir) = grammar_source(lang);
        let work = std::env::temp_dir()
            .join("luckybird-grammar-build")
            .join(repo.replace('/', "_"));

        if !work.exists() {
            let mut git = Command::new("git");
            git.args(["clone", "--depth", "1", &format!("https://github.com/{repo}.git")]).arg(&work);
            run(&mut git, "git clone")?;
        }

        let build_dir = if subdir.is_empty() { work.clone() } else { work.join(subdir) };
        std::fs::create_dir_all(&self.grammar_dir).map_err(|e| e.to_string())?;
        let final_path = self.grammar_dir.join(lb_syntax::loader::lib_filename(lang));

        let symbol = lb_syntax::registry::load()
            .get(lang)
            .map(|m| m.grammar_fn.clone())
            .ok_or_else(|| "unknown language".to_owned())?;
        let obj_dir = work.join(format!("obj-{lang}"));
        let built = obj_dir.join(lb_syntax::loader::lib_filename(lang));
        let _ = std::fs::remove_file(&built);
        compile_grammar(&build_dir, &symbol, &built, &obj_dir)?;
        if !built.exists() {
            return Err("build produced no library".to_owned());
        }
        swap_in(&built, &final_path)?;

        let query = [build_dir.join("queries"), work.join("queries")]
            .into_iter()
            .map(|d| d.join("highlights.scm"))
            .find(|p| p.exists());
        match query {
            Some(src) => {
                let dest_dir = self.query_dir.join(lang);
                std::fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
                std::fs::copy(&src, dest_dir.join("highlights.scm")).map_err(|e| e.to_string())?;
            }
            None => self.say(format!("{lang}: built, but the repo has no highlights.scm")),
        }
        Ok(())
    }

    fn install_server(&self, lang: &str) -> Result<(), String> {
        let steps = server_steps(lang)
            .ok_or_else(|| "no automatic installer; install its language server manually".to_owned())?;
        for step in steps {
            if let Some((program, args)) = step.split_first() {
                let mut cmd = tool(program);
                cmd.args(args);
                run(&mut cmd, program)?;
            }
        }
        Ok(())
    }
}

/// `npm`, `rustup`, ... may be `.cmd` shims on Windows, which only `cmd` can run.
fn tool(program: &str) -> Command {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", program]);
        cmd
    }
    #[cfg(not(windows))]
    {
        Command::new(program)
    }
}

pub fn run(cmd: &mut Command, label: &str) -> Result<(), String> {
    tracing::info!(command = ?cmd, "installer running");
    let out = cmd.stdin(Stdio::null()).output().map_err(|e| {
        if e.kind() == ErrorKind::NotFound {
            format!("{label}: not found on PATH")
        } else {
            format!("{label}: {e}")
        }
    })?;
    if out.status.success() {
        return Ok(());
    }
    tracing::warn!(
        label,
        status = %out.status,
        stdout = %String::from_utf8_lossy(&out.stdout),
        stderr = %String::from_utf8_lossy(&out.stderr),
        "installer command failed"
    );
    let mut text = String::from_utf8_lossy(&out.stderr).into_owned();
    if text.trim().is_empty() {
        text = String::from_utf8_lossy(&out.stdout).into_owned();
    }
    let tail = text.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("unknown error");
    let tail: String = tail.trim().chars().take(160).collect();
    Err(format!("{label} failed: {tail}"))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_and_override_repos() {
        assert_eq!(grammar_source("python"), ("tree-sitter/tree-sitter-python".to_owned(), ""));
        assert_eq!(
            grammar_source("tsx"),
            ("tree-sitter/tree-sitter-typescript".to_owned(), "tsx")
        );
    }

    /// Needs network, git and a C compiler: `cargo test -p lb-app -- --ignored`.
    #[test]
    #[ignore]
    fn builds_real_grammars_from_source() {
        let root = std::env::temp_dir().join("lb-installer-test");
        let _ = std::fs::remove_dir_all(&root);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let installer = Installer::new(tx, root.join("grammars"), root.join("queries"));
        for lang in ["json", "python"] {
            installer.install_grammar(lang).unwrap_or_else(|e| panic!("{lang}: {e}"));
            let lib = root.join("grammars").join(lb_syntax::loader::lib_filename(lang));
            assert!(lib.exists(), "{lib:?}");
            assert!(root.join("queries").join(lang).join("highlights.scm").exists());
            let symbol = format!("tree_sitter_{lang}");
            let mut loader = lb_syntax::GrammarLoader::new(root.join("grammars"));
            loader.load(lang, &symbol).unwrap_or_else(|e| panic!("load {lang}: {e}"));
            if lang == "python" {
                let mut syntax =
                    lb_syntax::SyntaxManager::new(root.join("grammars"), root.join("queries"));
                let mut layer = syntax.layer_for("python");
                assert!(layer.is_active(), "python layer should be active");
                let spans = layer.highlight("def f():\n    return 1\n");
                assert!(!spans.is_empty());
                let coloured = spans
                    .iter()
                    .filter(|s| layer.capture_name(s.capture).and_then(crate::theme::color_for).is_some())
                    .count();
                assert!(coloured >= 2, "expected keyword spans to get colours, got {coloured}");

                let text = "def foo():\n    return bar(1)\n";
                let spans = layer.highlight(text);
                let name_at = |start: usize, end: usize| {
                    spans
                        .iter()
                        .find(|s| s.start == start && s.end == end)
                        .and_then(|s| layer.capture_name(s.capture))
                };
                assert_eq!(name_at(4, 7), Some("function"), "def name: {spans:?}");
                assert_eq!(name_at(22, 25), Some("function"), "call name: {spans:?}");

                // Different function names get different colours; a name keeps its colour.
                let text = "def foo():\n    pass\n\ndef foo2():\n    pass\n\nfoo()\n";
                let coloured = crate::app::color_spans(&mut layer, text);
                let at = |start: usize| coloured.iter().find(|h| h.start == start).map(|h| h.rgb);
                let (def_foo, def_foo2, call_foo) = (at(4), at(25), at(43));
                assert!(def_foo.is_some() && def_foo2.is_some(), "{coloured:?}");
                assert_ne!(def_foo, def_foo2, "foo and foo2 must differ");
                assert_eq!(def_foo, call_foo, "foo() must match def foo");
            }
        }

        // Reinstall while the running editor still has the grammar loaded.
        let mut syntax = lb_syntax::SyntaxManager::new(root.join("grammars"), root.join("queries"));
        let _keep_loaded = syntax.layer_for("python");
        installer.install_grammar("python").unwrap_or_else(|e| panic!("reinstall: {e}"));
        syntax.forget_grammar("python");
        assert!(syntax.layer_for("python").is_active());
    }


    /// Needs network, git and a C compiler: `cargo test -p lb-app -- --ignored`.
    #[test]
    #[ignore]
    fn every_kind_of_name_gets_its_own_colour_in_each_language() {
        // (language, source, [(kind, first name, second name)])
        let cases: &[(&str, &str, &[(&str, &str, &str)])] = &[
            (
                "python",
                "class Alpha:\n    def foo(self, x):\n        total = x\n        return total\n\nclass Beta:\n    def foo2(self, y):\n        count = y\n        return count\n",
                &[("type", "Alpha", "Beta"), ("function", "foo", "foo2"), ("variable", "total", "count")],
            ),
            (
                "rust",
                "struct Alpha;\nstruct Beta;\nfn foo(x: i32) -> i32 {\n    let total = x;\n    total\n}\nfn foo2(y: i32) -> i32 {\n    let count = y;\n    count\n}\n",
                &[("type", "Alpha", "Beta"), ("function", "foo", "foo2"), ("variable", "total", "count")],
            ),
            (
                "javascript",
                "class Alpha {}\nclass Beta {}\nfunction foo(x) {\n  const total = x;\n  return total;\n}\nfunction foo2(y) {\n  const count = y;\n  return count;\n}\n",
                &[("type", "Alpha", "Beta"), ("function", "foo", "foo2"), ("variable", "total", "count")],
            ),
            (
                "go",
                "package main\n\ntype Alpha struct{}\ntype Beta struct{}\n\nfunc foo(x int) int {\n\ttotal := x\n\treturn total\n}\n\nfunc foo2(y int) int {\n\tcount := y\n\treturn count\n}\n",
                &[("type", "Alpha", "Beta"), ("function", "foo", "foo2"), ("variable", "total", "count")],
            ),
            (
                "c",
                "struct Alpha { int a; };\nstruct Beta { int b; };\nint foo(int x) {\n  int total = x;\n  return total;\n}\nint foo2(int y) {\n  int count = y;\n  return count;\n}\n",
                &[("type", "Alpha", "Beta"), ("function", "foo", "foo2"), ("variable", "total", "count")],
            ),
        ];

        let root = std::env::temp_dir().join("lb-installer-test-langs");
        let _ = std::fs::remove_dir_all(&root);
        let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
        let installer = Installer::new(tx, root.join("grammars"), root.join("queries"));
        let mut problems = Vec::new();

        for (lang, code, kinds) in cases {
            installer.install_grammar(lang).unwrap_or_else(|e| panic!("{lang}: {e}"));
            let mut syntax = lb_syntax::SyntaxManager::new(root.join("grammars"), root.join("queries"));
            let mut layer = syntax.layer_for(lang);
            assert!(layer.is_active(), "{lang}: layer not active");
            let raw = layer.highlight(code);
            let coloured = crate::app::color_spans(&mut layer, code);

            for (kind, a, b) in *kinds {
                let probe = |name: &str| {
                    let at = code.find(name).unwrap_or(usize::MAX);
                    let capture = raw
                        .iter()
                        .find(|s| s.start == at)
                        .and_then(|s| layer.capture_name(s.capture))
                        .unwrap_or("-")
                        .to_owned();
                    let rgb = coloured.iter().find(|h| h.start == at).map(|h| h.rgb);
                    (capture, rgb)
                };
                let (ca, ra) = probe(a);
                let (cb, rb) = probe(b);
                println!("{lang:<11} {kind:<9} {a:<6} @{ca:<22} {ra:?}  |  {b:<6} @{cb:<22} {rb:?}");
                if ra.is_none() || rb.is_none() || ra == rb {
                    problems.push(format!("{lang} {kind}: {a} ({ca}) {ra:?} vs {b} ({cb}) {rb:?}"));
                }
            }
        }
        let _ = std::fs::remove_dir_all(&root);
        assert!(problems.is_empty(), "names not distinguished:\n{}", problems.join("\n"));
    }
    #[test]
    fn only_registry_languages_are_known() {
        assert!(is_known("python"));
        assert!(!is_known("evil; rm -rf"));
    }

    #[test]
    fn server_table_covers_main_languages() {
        for lang in ["rust", "python", "javascript", "typescript", "go"] {
            assert!(server_steps(lang).is_some(), "{lang}");
        }
        assert!(server_steps("cobol").is_none());
    }
}
