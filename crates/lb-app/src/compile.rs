//! Compile a tree-sitter grammar's generated C sources straight into a shared
//! library, without the `tree-sitter` CLI.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::installer::run;

enum Toolchain {
    /// `cl` is already on PATH.
    MsvcOnPath,
    /// `cl` needs the environment set up by this `vcvars64.bat`.
    #[cfg(windows)]
    MsvcVcvars(PathBuf),
    /// A GNU-style pair: C compiler and C++ compiler.
    Unix(&'static str, &'static str),
}

pub fn compile_grammar(
    grammar_dir: &Path,
    symbol: &str,
    out: &Path,
    obj_dir: &Path,
) -> Result<(), String> {
    let src = grammar_dir.join("src");
    let parser = src.join("parser.c");
    if !parser.exists() {
        return Err("repo has no src/parser.c (its parser is not pre-generated)".to_owned());
    }
    let scanner = ["scanner.c", "scanner.cc"].iter().map(|n| src.join(n)).find(|p| p.exists());
    std::fs::create_dir_all(obj_dir).map_err(|e| e.to_string())?;
    if !symbol.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err("invalid grammar symbol".to_owned());
    }

    let mut sources = vec![parser];
    sources.extend(scanner);

    match detect()? {
        Toolchain::MsvcOnPath => {
            let mut cmd = Command::new("cl");
            cmd.args(["/nologo", "/O2", "/LD"]);
            cmd.arg(format!("/I{}", src.display()));
            cmd.args(&sources);
            cmd.arg(format!("/Fe{}", out.display()));
            cmd.args(["/link", &format!("/EXPORT:{symbol}")]);
            cmd.current_dir(obj_dir);
            run(&mut cmd, "cl")
        }
        #[cfg(windows)]
        Toolchain::MsvcVcvars(vcvars) => {
            use std::os::windows::process::CommandExt;
            for p in sources.iter().chain([&src, &vcvars.clone(), &out.to_path_buf()]) {
                let s = p.to_string_lossy();
                if s.contains(['"', '%', '&', '^', '!']) {
                    return Err(format!("path {s} has characters cmd.exe cannot pass safely"));
                }
            }
            let files: String = sources.iter().map(|p| format!(" \"{}\"", p.display())).collect();
            let line = format!(
                "/C \"\"{}\" >nul && cl /nologo /O2 /LD /I\"{}\"{files} /Fe\"{}\" /link /EXPORT:{symbol}\"",
                vcvars.display(),
                src.display(),
                out.display(),
            );
            let mut cmd = Command::new("cmd");
            cmd.raw_arg(line).current_dir(obj_dir);
            run(&mut cmd, "cl")
        }
        Toolchain::Unix(cc, cxx) => {
            let mut objects = Vec::new();
            let mut needs_cxx = false;
            for source in &sources {
                let is_cxx = source.extension().is_some_and(|e| e == "cc");
                needs_cxx |= is_cxx;
                let stem = source.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let object = obj_dir.join(format!("{stem}.o"));
                let mut cmd = Command::new(if is_cxx { cxx } else { cc });
                cmd.args(["-fPIC", "-O2", "-c"]).arg(format!("-I{}", src.display()));
                cmd.arg(source).arg("-o").arg(&object);
                run(&mut cmd, cc)?;
                objects.push(object);
            }
            let mut link = Command::new(if needs_cxx { cxx } else { cc });
            link.arg("-shared").arg("-o").arg(out).args(&objects);
            run(&mut link, cc)
        }
    }
}

fn detect() -> Result<Toolchain, String> {
    #[cfg(windows)]
    if Command::new("cl").output().is_ok() {
        return Ok(Toolchain::MsvcOnPath);
    }
    for (cc, cxx) in [("gcc", "g++"), ("clang", "clang++"), ("cc", "c++")] {
        if Command::new(cc).arg("--version").output().is_ok() {
            return Ok(Toolchain::Unix(cc, cxx));
        }
    }
    #[cfg(windows)]
    if let Some(vcvars) = find_vcvars() {
        return Ok(Toolchain::MsvcVcvars(vcvars));
    }
    Err("no C compiler found (install Visual Studio Build Tools with the C++ workload, gcc or clang)".to_owned())
}

#[cfg(windows)]
fn find_vcvars() -> Option<PathBuf> {
    let program_files = std::env::var_os("ProgramFiles(x86)")?;
    let vswhere = PathBuf::from(program_files)
        .join("Microsoft Visual Studio")
        .join("Installer")
        .join("vswhere.exe");
    let out = Command::new(vswhere)
        .args([
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ])
        .output()
        .ok()?;
    let root = String::from_utf8_lossy(&out.stdout).lines().next()?.trim().to_owned();
    let vcvars = PathBuf::from(root).join("VC").join("Auxiliary").join("Build").join("vcvars64.bat");
    vcvars.exists().then_some(vcvars)
}
