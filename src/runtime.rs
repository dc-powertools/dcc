use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::Context as _;

use crate::cache::CacheDir;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum ContainerMode {
    OneShot,
    Durable,
}

impl ContainerMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::OneShot => "oneshot",
            Self::Durable => "durable",
        }
    }

    fn parse(value: &str) -> Self {
        if value.trim() == "durable" {
            Self::Durable
        } else {
            Self::OneShot
        }
    }
}

#[derive(Debug)]
pub(crate) struct RuntimeState {
    root: PathBuf,
}

#[derive(Debug)]
pub(crate) struct ActiveCommand {
    path: PathBuf,
}

#[derive(Debug)]
pub(crate) struct RuntimeLock {
    path: PathBuf,
}

impl RuntimeState {
    pub(crate) fn new(cache_dir: &CacheDir) -> Self {
        Self {
            root: cache_dir.host_path.join("runtime"),
        }
    }

    pub(crate) fn ensure_exists(&self) -> anyhow::Result<()> {
        fs::create_dir_all(self.active_dir()).with_context(|| {
            format!(
                "failed to create runtime state directory `{}`",
                self.root.display()
            )
        })
    }

    pub(crate) fn acquire_lock(&self) -> anyhow::Result<RuntimeLock> {
        self.ensure_exists()?;
        let lock_path = self.root.join("lock");
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match fs::create_dir(&lock_path) {
                Ok(()) => {
                    return Ok(RuntimeLock { path: lock_path });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    if Instant::now() >= deadline {
                        anyhow::bail!(
                            "timed out waiting for runtime lock `{}`",
                            lock_path.display()
                        );
                    }
                    std::thread::sleep(Duration::from_millis(25));
                }
                Err(e) => {
                    return Err(e).with_context(|| {
                        format!("failed to create runtime lock `{}`", lock_path.display())
                    });
                }
            }
        }
    }

    pub(crate) fn create_active_command(&self) -> anyhow::Result<ActiveCommand> {
        self.ensure_exists()?;
        let pid = std::process::id();
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = self.active_dir().join(format!("{pid}-{nanos}.active"));
        fs::write(&path, pid.to_string()).with_context(|| {
            format!(
                "failed to create active command record `{}`",
                path.display()
            )
        })?;
        Ok(ActiveCommand { path })
    }

    pub(crate) fn complete_active_command(&self, command: &ActiveCommand) -> anyhow::Result<()> {
        match fs::remove_file(&command.path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e).with_context(|| {
                format!(
                    "failed to remove active command record `{}`",
                    command.path.display()
                )
            }),
        }
    }

    pub(crate) fn set_mode(&self, mode: ContainerMode) -> anyhow::Result<()> {
        self.ensure_exists()?;
        fs::write(self.mode_path(), mode.as_str()).with_context(|| {
            format!(
                "failed to write runtime mode `{}`",
                self.mode_path().display()
            )
        })
    }

    pub(crate) fn mode(&self) -> ContainerMode {
        fs::read_to_string(self.mode_path())
            .map(|value| ContainerMode::parse(&value))
            .unwrap_or(ContainerMode::OneShot)
    }

    pub(crate) fn active_count(&self) -> anyhow::Result<usize> {
        self.cleanup_stale_active_commands()?;
        let mut count = 0usize;
        for entry in fs::read_dir(self.active_dir()).with_context(|| {
            format!(
                "failed to read active command directory `{}`",
                self.active_dir().display()
            )
        })? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                count += 1;
            }
        }
        Ok(count)
    }

    pub(crate) fn clear(&self) -> anyhow::Result<()> {
        if !self.root.exists() {
            return Ok(());
        }
        fs::remove_dir_all(&self.root).with_context(|| {
            format!(
                "failed to remove runtime state directory `{}`",
                self.root.display()
            )
        })
    }

    fn cleanup_stale_active_commands(&self) -> anyhow::Result<()> {
        for entry in fs::read_dir(self.active_dir()).with_context(|| {
            format!(
                "failed to read active command directory `{}`",
                self.active_dir().display()
            )
        })? {
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let path = entry.path();
            let Ok(pid_text) = fs::read_to_string(&path) else {
                continue;
            };
            let Ok(pid) = pid_text.trim().parse::<u32>() else {
                continue;
            };
            if !process_is_alive(pid) {
                let _ = fs::remove_file(path);
            }
        }
        Ok(())
    }

    fn active_dir(&self) -> PathBuf {
        self.root.join("active")
    }

    fn mode_path(&self) -> PathBuf {
        self.root.join("mode")
    }
}

impl Drop for RuntimeLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.path);
    }
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .map(|status| status.success())
        .unwrap_or(true)
}

#[cfg(not(unix))]
fn process_is_alive(_pid: u32) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn state(root: &Path) -> RuntimeState {
        RuntimeState {
            root: root.join("runtime"),
        }
    }

    #[test]
    fn mode_defaults_to_oneshot() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(state(tmp.path()).mode(), ContainerMode::OneShot);
    }

    #[test]
    fn mode_round_trips_durable() {
        let tmp = tempfile::tempdir().unwrap();
        let state = state(tmp.path());
        state.set_mode(ContainerMode::Durable).unwrap();
        assert_eq!(state.mode(), ContainerMode::Durable);
    }

    #[test]
    fn active_command_records_count_and_complete() {
        let tmp = tempfile::tempdir().unwrap();
        let state = state(tmp.path());
        let _lock = state.acquire_lock().unwrap();
        let command = state.create_active_command().unwrap();
        assert_eq!(state.active_count().unwrap(), 1);
        state.complete_active_command(&command).unwrap();
        assert_eq!(state.active_count().unwrap(), 0);
    }

    #[test]
    fn clear_removes_runtime_state() {
        let tmp = tempfile::tempdir().unwrap();
        let state = state(tmp.path());
        state.set_mode(ContainerMode::Durable).unwrap();
        assert!(state.root.exists());
        state.clear().unwrap();
        assert!(!state.root.exists());
    }

    #[test]
    fn lock_excludes_second_holder() {
        let tmp = tempfile::tempdir().unwrap();
        let state = state(tmp.path());
        let lock = state.acquire_lock().unwrap();
        assert!(state.root.join("lock").is_dir());
        drop(lock);
        assert!(state.acquire_lock().is_ok());
    }
}
