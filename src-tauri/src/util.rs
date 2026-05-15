//! Tiny shared utilities. Currently just process-spawn helpers that suppress
//! the brief CMD console that Windows pops when you `Command::new("sc")` or
//! similar CLI-tool invocations from a Tauri GUI process.
//!
//! On a normal GUI app every `std::process::Command::new(...).output()`
//! for a console subsystem binary (sc.exe, powershell.exe, moonlight pair)
//! creates a new console window for the child. Without intervention it
//! flashes onscreen for a few hundred ms — invisible if it happens once
//! at startup, very visible if some UI polls service state every 3
//! seconds. Setting `CREATE_NO_WINDOW` on the child's creation flags
//! suppresses the console association entirely.
//!
//! Use [`silent_cmd_std`] / [`silent_cmd_tokio`] for any CLI tool we
//! invoke for housekeeping. Emulators we launch via `orchestration::launcher`
//! deliberately keep their own consoles, so that path doesn't use these.

#[cfg(target_os = "windows")]
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// `std::process::Command::new(program)` with `CREATE_NO_WINDOW` already
/// applied on Windows. No-op on other platforms.
pub fn silent_cmd_std(program: impl AsRef<std::ffi::OsStr>) -> std::process::Command {
    let mut c = std::process::Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}

/// Tokio variant — same idea, for `.spawn()` / `.status()` / `.output()`
/// callers that need an async-aware Command.
pub fn silent_cmd_tokio(program: impl AsRef<std::ffi::OsStr>) -> tokio::process::Command {
    let mut c = tokio::process::Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(CREATE_NO_WINDOW);
    }
    c
}
