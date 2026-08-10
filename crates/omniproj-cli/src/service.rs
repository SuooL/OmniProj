//! Persistent-daemon service management (W2-1, spec §7 "守护进程生命周期").
//!
//! `omniproj install-service` registers `omniproj daemon` with the OS supervisor so it
//! starts at login and restarts on crash: a **launchd LaunchAgent** on macOS and a
//! **systemd user unit** on Linux. `omniproj uninstall-service` tears it back down. Other
//! platforms get a clear error and are told to run `omniproj daemon` manually.
//!
//! The file-content builders ([`launchd_plist`] / `systemd_unit`) are pure functions
//! so they can be unit-tested without touching the real filesystem or `launchctl`.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

/// launchd label / systemd unit basename. Kept stable so re-installs are idempotent.
/// Both are always defined (the pure content-builders + their tests compile on every
/// platform); each is only *used* on its own OS, hence `allow(dead_code)`.
#[allow(dead_code)]
const LAUNCHD_LABEL: &str = "com.omniproj.daemon";
#[allow(dead_code)]
const SYSTEMD_UNIT: &str = "omniproj.service";

/// Absolute path of the currently-running `omniproj` binary, embedded into the unit so
/// the supervisor invokes exactly this build (not whatever is on `$PATH` at boot).
fn current_exe() -> Result<String> {
    let exe = std::env::current_exe().context("locate omniproj executable")?;
    Ok(exe.to_string_lossy().into_owned())
}

/// The daemon's stdout/stderr log — same path the lazy-spawn path uses.
fn log_path() -> String {
    omniproj_core::omniproj_home()
        .join("daemon.log")
        .to_string_lossy()
        .into_owned()
}

/// Render the macOS LaunchAgent plist. `RunAtLoad` starts it at login; `KeepAlive`
/// restarts it if it ever exits (crash recovery, W2-1).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn launchd_plist(exe: &str, log_path: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LAUNCHD_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{log_path}</string>
    <key>StandardErrorPath</key>
    <string>{log_path}</string>
</dict>
</plist>
"#
    )
}

/// Render the systemd **user** unit. `Restart=always` + `RestartSec=5` provide crash
/// recovery; `WantedBy=default.target` starts it at user login (W2-1).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn systemd_unit(exe: &str) -> String {
    format!(
        "[Unit]\n\
         Description=OmniProj background daemon (cognitive scaffolding)\n\
         After=default.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={exe} daemon\n\
         Restart=always\n\
         RestartSec=5\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    )
}

#[cfg(target_os = "macos")]
fn launchd_plist_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not resolve home directory")?;
    Ok(home
        .join("Library")
        .join("LaunchAgents")
        .join(format!("{LAUNCHD_LABEL}.plist")))
}

#[cfg(target_os = "linux")]
fn systemd_unit_path() -> Result<PathBuf> {
    let dir = dirs::config_dir().context("could not resolve XDG config directory")?;
    Ok(dir.join("systemd").join("user").join(SYSTEMD_UNIT))
}

/// The uid of the current user, for `launchctl bootstrap gui/<uid>`.
#[cfg(target_os = "macos")]
fn current_uid() -> Result<String> {
    let out = Command::new("id")
        .arg("-u")
        .output()
        .context("run `id -u`")?;
    let uid = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if uid.is_empty() {
        anyhow::bail!("could not determine current uid");
    }
    Ok(uid)
}

/// Install and start the persistent daemon service for the current platform.
#[allow(clippy::needless_return)] // cfg-gated blocks: `return` keeps each platform arm self-contained
pub fn install_service() -> Result<()> {
    omniproj_core::ensure_home()?;
    let exe = current_exe()?;

    #[cfg(target_os = "macos")]
    {
        let plist = launchd_plist_path()?;
        if let Some(parent) = plist.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        std::fs::write(&plist, launchd_plist(&exe, &log_path()))
            .with_context(|| format!("write {}", plist.display()))?;
        println!("[omniproj] wrote LaunchAgent {}", plist.display());

        let uid = current_uid()?;
        let domain = format!("gui/{uid}");
        // Idempotent: drop any prior registration before (re)loading. Errors here are
        // expected when nothing was loaded yet, so they're informational only.
        let _ = Command::new("launchctl")
            .args(["bootout", &domain])
            .arg(&plist)
            .status();
        let bootstrap = Command::new("launchctl")
            .args(["bootstrap", &domain])
            .arg(&plist)
            .status()
            .context("run `launchctl bootstrap`")?;
        if !bootstrap.success() {
            // Older macOS (pre-Yosemite semantics) or edge cases: fall back to load -w.
            let loaded = Command::new("launchctl")
                .args(["load", "-w"])
                .arg(&plist)
                .status()
                .context("run `launchctl load -w`")?;
            if !loaded.success() {
                anyhow::bail!(
                    "launchctl could not load {} — load it manually or run `omniproj daemon`",
                    plist.display()
                );
            }
        }
        println!("[omniproj] daemon service loaded (starts at login, restarts on crash)");
        println!("[omniproj] logs: {}", log_path());
        println!("[omniproj] check status: launchctl list | grep omniproj");
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let unit = systemd_unit_path()?;
        if let Some(parent) = unit.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create {}", parent.display()))?;
        }
        std::fs::write(&unit, systemd_unit(&exe))
            .with_context(|| format!("write {}", unit.display()))?;
        println!("[omniproj] wrote systemd user unit {}", unit.display());

        run_systemctl(&["daemon-reload"])?;
        run_systemctl(&["enable", "--now", SYSTEMD_UNIT])?;
        println!("[omniproj] daemon service enabled (starts at login, restarts on crash)");
        println!("[omniproj] logs: {}", log_path());
        println!("[omniproj] check status: systemctl --user status omniproj");
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = exe;
        anyhow::bail!(
            "service install is only supported on macOS and Linux; run `omniproj daemon` manually"
        )
    }
}

/// Stop and remove the persistent daemon service for the current platform.
#[allow(clippy::needless_return)] // cfg-gated blocks: `return` keeps each platform arm self-contained
pub fn uninstall_service() -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        let plist = launchd_plist_path()?;
        if let Ok(uid) = current_uid() {
            let domain = format!("gui/{uid}");
            let _ = Command::new("launchctl")
                .args(["bootout", &domain])
                .arg(&plist)
                .status();
        }
        // Fallback unload for older macOS; harmless if bootout already handled it.
        let _ = Command::new("launchctl")
            .args(["unload", "-w"])
            .arg(&plist)
            .status();
        if plist.exists() {
            std::fs::remove_file(&plist).with_context(|| format!("remove {}", plist.display()))?;
            println!("[omniproj] removed LaunchAgent {}", plist.display());
        } else {
            println!("[omniproj] no LaunchAgent installed at {}", plist.display());
        }
        println!("[omniproj] daemon service uninstalled");
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        let unit = systemd_unit_path()?;
        // disable --now stops it and removes the enable symlink; ignore errors (may be
        // already stopped / never enabled).
        let _ = run_systemctl(&["disable", "--now", SYSTEMD_UNIT]);
        if unit.exists() {
            std::fs::remove_file(&unit).with_context(|| format!("remove {}", unit.display()))?;
            println!("[omniproj] removed systemd user unit {}", unit.display());
        } else {
            println!("[omniproj] no systemd user unit at {}", unit.display());
        }
        let _ = run_systemctl(&["daemon-reload"]);
        println!("[omniproj] daemon service uninstalled");
        return Ok(());
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        anyhow::bail!("service management is only supported on macOS and Linux")
    }
}

#[cfg(target_os = "linux")]
fn run_systemctl(args: &[&str]) -> Result<()> {
    let status = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .status()
        .with_context(|| format!("run `systemctl --user {}`", args.join(" ")))?;
    if !status.success() {
        anyhow::bail!("`systemctl --user {}` failed", args.join(" "));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launchd_plist_embeds_exe_daemon_keepalive_and_log() {
        let p = launchd_plist("/usr/local/bin/omniproj", "/home/u/.omniproj/daemon.log");
        assert!(p.contains("<string>/usr/local/bin/omniproj</string>"));
        assert!(p.contains("<string>daemon</string>"));
        // crash recovery + login start
        assert!(p.contains("<key>KeepAlive</key>"));
        assert!(p.contains("<key>RunAtLoad</key>"));
        // logs routed to the daemon log
        assert!(p.contains("<string>/home/u/.omniproj/daemon.log</string>"));
        assert!(p.contains("com.omniproj.daemon"));
        // well-formed enough: single plist root
        assert!(p.starts_with("<?xml"));
    }

    #[test]
    fn systemd_unit_has_execstart_restart_and_install_target() {
        let u = systemd_unit("/usr/local/bin/omniproj");
        assert!(u.contains("ExecStart=/usr/local/bin/omniproj daemon"));
        // crash recovery
        assert!(u.contains("Restart=always"));
        assert!(u.contains("RestartSec=5"));
        // login start
        assert!(u.contains("WantedBy=default.target"));
        assert!(u.contains("[Service]"));
        assert!(u.contains("[Install]"));
    }
}
