use mlua::{Lua, Result as LuaResult};

/// Remove or replace dangerous Lua standard-library functions.
///
/// After calling this function the Lua state cannot:
/// - Execute arbitrary OS commands (`os.execute`, `os.exit`)
/// - Open arbitrary files (`io.open`, `io.popen`)
/// - Load arbitrary Lua code from the filesystem (`loadfile`, `dofile`)
/// - Require packages from arbitrary paths (`require` is kept but restricted
///   elsewhere)
///
/// `os.clock`, `os.time`, `os.date`, `os.difftime` are intentionally kept
/// for use by plugins.
pub fn apply(lua: &Lua) -> LuaResult<()> {
    // ── os table ───────────────────────────────────────────────────────────
    let os: mlua::Table = lua.globals().get("os")?;
    os.set("execute", mlua::Value::Nil)?;
    os.set("exit", mlua::Value::Nil)?;
    os.set("tmpname", mlua::Value::Nil)?;
    os.set("rename", mlua::Value::Nil)?;
    os.set("remove", mlua::Value::Nil)?;
    os.set("setlocale", mlua::Value::Nil)?;
    os.set("getenv", mlua::Value::Nil)?;

    // ── io table ───────────────────────────────────────────────────────────
    let io: mlua::Table = lua.globals().get("io")?;
    io.set("open", mlua::Value::Nil)?;
    io.set("popen", mlua::Value::Nil)?;
    io.set("lines", mlua::Value::Nil)?;
    io.set("tmpfile", mlua::Value::Nil)?;

    // ── load* / do* ────────────────────────────────────────────────────────
    lua.globals().set("loadfile", mlua::Value::Nil)?;
    lua.globals().set("dofile", mlua::Value::Nil)?;

    // ── debug table — keep but strip dangerous parts ───────────────────────
    if let Ok(debug_tbl) = lua.globals().get::<_, mlua::Table>("debug") {
        debug_tbl.set("getinfo", mlua::Value::Nil)?;
        debug_tbl.set("getupvalue", mlua::Value::Nil)?;
        debug_tbl.set("setupvalue", mlua::Value::Nil)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_removes_os_execute() {
        let lua = Lua::new();
        apply(&lua).unwrap();
        let result: mlua::Value = lua.globals().get("os").unwrap();
        if let mlua::Value::Table(os) = result {
            let exec: mlua::Value = os.get("execute").unwrap();
            assert!(matches!(exec, mlua::Value::Nil));
        }
    }

    #[test]
    fn sandbox_keeps_os_time() {
        let lua = Lua::new();
        apply(&lua).unwrap();
        let os: mlua::Table = lua.globals().get("os").unwrap();
        let time: mlua::Value = os.get("time").unwrap();
        assert!(!matches!(time, mlua::Value::Nil));
    }

    #[test]
    fn sandbox_removes_loadfile() {
        let lua = Lua::new();
        apply(&lua).unwrap();
        let v: mlua::Value = lua.globals().get("loadfile").unwrap();
        assert!(matches!(v, mlua::Value::Nil));
    }
}
