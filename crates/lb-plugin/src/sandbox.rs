use mlua::{Lua, LuaOptions, Result as LuaResult, StdLib, Table, Value};

const OS_ALLOWED: [&str; 4] = ["clock", "time", "date", "difftime"];

/// Create a Lua state that only has the pure standard libraries.
///
/// `io`, `package` (so `require` and `package.loadlib`) and `debug` are never
/// loaded; a blocklist on them would leave escape hatches such as
/// `io.output(path)` or `package.loadlib`.
pub fn new_lua() -> LuaResult<Lua> {
    let libs = StdLib::COROUTINE | StdLib::TABLE | StdLib::STRING | StdLib::UTF8 | StdLib::MATH
        | StdLib::OS;
    let lua = Lua::new_with(libs, LuaOptions::default())?;
    apply(&lua)?;
    Ok(lua)
}

/// Reduce an existing Lua state to the safe subset.
///
/// Keeps only `os.clock/time/date/difftime`, removes everything that can touch
/// the filesystem, load native code or inspect the VM, and restricts `load` to
/// text chunks (precompiled bytecode can crash the VM).
pub fn apply(lua: &Lua) -> LuaResult<()> {
    let globals = lua.globals();

    let safe_os = lua.create_table()?;
    if let Ok(os) = globals.get::<_, Table>("os") {
        for name in OS_ALLOWED {
            let f: Value = os.get(name)?;
            safe_os.set(name, f)?;
        }
    }
    globals.set("os", safe_os)?;

    for name in [
        "io", "debug", "package", "require", "loadfile", "dofile", "loadstring", "collectgarbage",
    ] {
        globals.set(name, Value::Nil)?;
    }

    let text_only_load: mlua::Function = lua
        .load(
            "local raw = load\n\
             return function(chunk, name, _mode, env)\n\
                 return raw(chunk, name, 't', env)\n\
             end",
        )
        .eval()?;
    globals.set("load", text_only_load)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_nil(lua: &Lua, expr: &str) -> bool {
        lua.load(expr).eval::<Value>().map(|v| matches!(v, Value::Nil)).unwrap_or(false)
    }

    #[test]
    fn sandbox_removes_os_execute() {
        let lua = new_lua().unwrap();
        assert!(is_nil(&lua, "return os.execute"));
        assert!(is_nil(&lua, "return os.getenv"));
        assert!(is_nil(&lua, "return os.remove"));
    }

    #[test]
    fn sandbox_keeps_os_time() {
        let lua = new_lua().unwrap();
        assert!(!is_nil(&lua, "return os.time"));
    }

    #[test]
    fn sandbox_removes_loadfile() {
        let lua = new_lua().unwrap();
        assert!(is_nil(&lua, "return loadfile"));
        assert!(is_nil(&lua, "return dofile"));
    }

    #[test]
    fn sandbox_has_no_io_package_or_debug() {
        let lua = new_lua().unwrap();
        for g in ["io", "package", "require", "debug"] {
            assert!(is_nil(&lua, &format!("return {g}")), "{g} must be absent");
        }
    }

    #[test]
    fn apply_hardens_a_full_state() {
        let lua = Lua::new();
        apply(&lua).unwrap();
        assert!(is_nil(&lua, "return io"));
        assert!(is_nil(&lua, "return package"));
        assert!(is_nil(&lua, "return debug"));
    }

    #[test]
    fn load_rejects_binary_chunks() {
        let lua = new_lua().unwrap();
        let ok: bool = lua
            .load("local f = load('\\27Lua') return f == nil")
            .eval()
            .unwrap();
        assert!(ok);
        let works: i64 = lua.load("return load('return 1 + 1')()").eval().unwrap();
        assert_eq!(works, 2);
    }
}
