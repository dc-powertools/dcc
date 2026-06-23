use std::path::Path;
use std::process::{ExitStatus, Stdio};

use anyhow::Context as _;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::docker;

/// A single named command within an object-form lifecycle hook: a shell
/// string (run via `/bin/sh -c`) or an argv array (executed directly).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum LifecycleCommandSingle {
    Shell(String),
    Exec(Vec<String>),
}

impl LifecycleCommandSingle {
    fn into_argv(self) -> Vec<String> {
        match self {
            Self::Shell(s) => vec!["/bin/sh".to_string(), "-c".to_string(), s],
            Self::Exec(argv) => argv,
        }
    }

    fn substitute(&self, f: &impl Fn(&str) -> String) -> Self {
        match self {
            Self::Shell(s) => Self::Shell(f(s)),
            Self::Exec(argv) => Self::Exec(argv.iter().map(|s| f(s)).collect()),
        }
    }

    fn try_substitute(&self, f: &impl Fn(&str) -> anyhow::Result<String>) -> anyhow::Result<Self> {
        Ok(match self {
            Self::Shell(s) => Self::Shell(f(s)?),
            Self::Exec(argv) => Self::Exec(
                argv.iter()
                    .map(|s| f(s))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            ),
        })
    }
}

/// A devcontainer lifecycle hook value: a shell string, an argv array, or a
/// map of named commands that run in parallel (blocking subsequent hooks
/// until all of them finish).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum LifecycleCommand {
    Shell(String),
    Exec(Vec<String>),
    Parallel(IndexMap<String, LifecycleCommandSingle>),
}

impl LifecycleCommand {
    /// Returns one argv per command to run. The object form yields one argv
    /// per entry, to be run in parallel. Empty argvs (e.g. `Exec(vec![])`)
    /// are filtered out, since they are no-ops.
    pub(crate) fn argvs(&self) -> Vec<Vec<String>> {
        let raw = match self {
            Self::Shell(s) => vec![LifecycleCommandSingle::Shell(s.clone()).into_argv()],
            Self::Exec(argv) => vec![argv.clone()],
            Self::Parallel(map) => map
                .values()
                .cloned()
                .map(LifecycleCommandSingle::into_argv)
                .collect(),
        };
        raw.into_iter().filter(|argv| !argv.is_empty()).collect()
    }

    /// Maps `f` over every string leaf (shell command text, argv elements,
    /// or values of an object-form map).
    pub(crate) fn substitute(&self, f: &impl Fn(&str) -> String) -> Self {
        match self {
            Self::Shell(s) => Self::Shell(f(s)),
            Self::Exec(argv) => Self::Exec(argv.iter().map(|s| f(s)).collect()),
            Self::Parallel(map) => Self::Parallel(
                map.iter()
                    .map(|(k, v)| (k.clone(), v.substitute(f)))
                    .collect(),
            ),
        }
    }

    /// Fallible mirror of [`substitute`](Self::substitute): maps `f` over every
    /// string leaf, returning the first error. Used for run-time
    /// `${containerEnv:…}` resolution, which can fail on an undefined/empty variable.
    pub(crate) fn try_substitute(
        &self,
        f: &impl Fn(&str) -> anyhow::Result<String>,
    ) -> anyhow::Result<Self> {
        Ok(match self {
            Self::Shell(s) => Self::Shell(f(s)?),
            Self::Exec(argv) => Self::Exec(
                argv.iter()
                    .map(|s| f(s))
                    .collect::<anyhow::Result<Vec<_>>>()?,
            ),
            Self::Parallel(map) => {
                let mut out = IndexMap::new();
                for (k, v) in map {
                    out.insert(k.clone(), v.try_substitute(f)?);
                }
                Self::Parallel(out)
            }
        })
    }
}

/// The five container-side lifecycle hooks shared by `devcontainer.json` and
/// `devcontainer-feature.json`, in their spec-defined execution order (see
/// [`HOOKS`]).
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct LifecycleHooks {
    pub(crate) on_create_command: Option<LifecycleCommand>,
    pub(crate) update_content_command: Option<LifecycleCommand>,
    pub(crate) post_create_command: Option<LifecycleCommand>,
    pub(crate) post_start_command: Option<LifecycleCommand>,
    pub(crate) post_attach_command: Option<LifecycleCommand>,
}

impl LifecycleHooks {
    pub(crate) fn substitute(&self, f: &impl Fn(&str) -> String) -> Self {
        Self {
            on_create_command: self.on_create_command.as_ref().map(|c| c.substitute(f)),
            update_content_command: self
                .update_content_command
                .as_ref()
                .map(|c| c.substitute(f)),
            post_create_command: self.post_create_command.as_ref().map(|c| c.substitute(f)),
            post_start_command: self.post_start_command.as_ref().map(|c| c.substitute(f)),
            post_attach_command: self.post_attach_command.as_ref().map(|c| c.substitute(f)),
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.on_create_command.is_none()
            && self.update_content_command.is_none()
            && self.post_create_command.is_none()
            && self.post_start_command.is_none()
            && self.post_attach_command.is_none()
    }
}

/// Reads one hook field out of a [`LifecycleHooks`].
type HookAccessor = fn(&LifecycleHooks) -> &Option<LifecycleCommand>;

/// The five container-side hooks in execution order, paired with their
/// `devcontainer.json` property name (for error messages) and an accessor
/// into [`LifecycleHooks`].
pub(crate) const HOOKS: [(&str, HookAccessor); 5] = [
    ("onCreateCommand", |h| &h.on_create_command),
    ("updateContentCommand", |h| &h.update_content_command),
    ("postCreateCommand", |h| &h.post_create_command),
    ("postStartCommand", |h| &h.post_start_command),
    ("postAttachCommand", |h| &h.post_attach_command),
];

/// Runs `cmd` inside `container` as `user` from `workdir` via `docker exec`.
/// Object-form commands run in parallel; if any fail, the first error (in
/// map order) is returned once all parallel commands have finished.
pub(crate) async fn run_in_container(
    cmd: &LifecycleCommand,
    container: &str,
    user: &str,
    workdir: &str,
) -> anyhow::Result<()> {
    let argvs = cmd.argvs();
    if argvs.is_empty() {
        return Ok(());
    }
    if let [argv] = argvs.as_slice() {
        let status = docker::exec(container, user, workdir, argv).await?;
        return docker::check_status(status, &argv.join(" "));
    }

    let mut handles = Vec::with_capacity(argvs.len());
    for argv in argvs {
        let container = container.to_owned();
        let user = user.to_owned();
        let workdir = workdir.to_owned();
        handles.push(tokio::spawn(async move {
            let status = docker::exec(&container, &user, &workdir, &argv).await?;
            docker::check_status(status, &argv.join(" "))
        }));
    }
    join_all(handles).await
}

/// Runs `cmd` on the host with working directory `cwd`. Same parallel
/// semantics as [`run_in_container`].
pub(crate) async fn run_on_host(cmd: &LifecycleCommand, cwd: &Path) -> anyhow::Result<()> {
    let argvs = cmd.argvs();
    if argvs.is_empty() {
        return Ok(());
    }
    if let [argv] = argvs.as_slice() {
        let status = exec_on_host(cwd, argv).await?;
        return docker::check_status(status, &argv.join(" "));
    }

    let mut handles = Vec::with_capacity(argvs.len());
    for argv in argvs {
        let cwd = cwd.to_owned();
        handles.push(tokio::spawn(async move {
            let status = exec_on_host(&cwd, &argv).await?;
            docker::check_status(status, &argv.join(" "))
        }));
    }
    join_all(handles).await
}

/// Awaits every handle, returning the first `Err` (in handle order) once all
/// have completed, or `Ok(())` if every command succeeded.
async fn join_all(handles: Vec<tokio::task::JoinHandle<anyhow::Result<()>>>) -> anyhow::Result<()> {
    let mut first_error = None;
    for handle in handles {
        let result = handle.await.context("lifecycle command task panicked")?;
        if let Err(e) = result {
            first_error.get_or_insert(e);
        }
    }
    match first_error {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

async fn exec_on_host(cwd: &Path, argv: &[String]) -> anyhow::Result<ExitStatus> {
    let program = argv.first().context("empty lifecycle command")?;
    Command::new(program)
        .args(&argv[1..])
        .current_dir(cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .with_context(|| format!("failed to spawn `{}`", argv.join(" ")))?
        .wait()
        .await
        .with_context(|| format!("failed to wait for `{}`", argv.join(" ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lifecycle_command(json: &str) -> LifecycleCommand {
        serde_json::from_str(json).unwrap()
    }

    // --- LifecycleCommand deserialization ---

    #[test]
    fn deserialize_shell_string() {
        assert_eq!(
            lifecycle_command(r#""echo hello""#),
            LifecycleCommand::Shell("echo hello".to_string())
        );
    }

    #[test]
    fn deserialize_exec_array() {
        assert_eq!(
            lifecycle_command(r#"["echo", "hello"]"#),
            LifecycleCommand::Exec(vec!["echo".to_string(), "hello".to_string()])
        );
    }

    #[test]
    fn deserialize_parallel_object() {
        let cmd = lifecycle_command(r#"{"a": "echo a", "b": ["echo", "b"]}"#);
        match cmd {
            LifecycleCommand::Parallel(map) => {
                assert_eq!(
                    map.get("a"),
                    Some(&LifecycleCommandSingle::Shell("echo a".to_string()))
                );
                assert_eq!(
                    map.get("b"),
                    Some(&LifecycleCommandSingle::Exec(vec![
                        "echo".to_string(),
                        "b".to_string()
                    ]))
                );
            }
            other => panic!("expected Parallel, got {other:?}"),
        }
    }

    // --- LifecycleCommand::argvs ---

    #[test]
    fn argvs_shell_wraps_in_sh_c() {
        let cmd = LifecycleCommand::Shell("echo hi".to_string());
        assert_eq!(
            cmd.argvs(),
            vec![vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "echo hi".to_string()
            ]]
        );
    }

    #[test]
    fn argvs_exec_passthrough() {
        let cmd = LifecycleCommand::Exec(vec!["echo".to_string(), "hi".to_string()]);
        assert_eq!(
            cmd.argvs(),
            vec![vec!["echo".to_string(), "hi".to_string()]]
        );
    }

    #[test]
    fn argvs_empty_exec_filtered_out() {
        let cmd = LifecycleCommand::Exec(vec![]);
        assert_eq!(cmd.argvs(), Vec::<Vec<String>>::new());
    }

    #[test]
    fn argvs_parallel_multiple_entries() {
        let mut map = IndexMap::new();
        map.insert(
            "a".to_string(),
            LifecycleCommandSingle::Exec(vec!["echo".to_string(), "a".to_string()]),
        );
        map.insert(
            "b".to_string(),
            LifecycleCommandSingle::Shell("echo b".to_string()),
        );
        let cmd = LifecycleCommand::Parallel(map);
        let argvs = cmd.argvs();
        assert_eq!(argvs.len(), 2);
        assert_eq!(argvs[0], vec!["echo".to_string(), "a".to_string()]);
        assert_eq!(
            argvs[1],
            vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "echo b".to_string()
            ]
        );
    }

    #[test]
    fn argvs_parallel_filters_empty_entries() {
        let mut map = IndexMap::new();
        map.insert("empty".to_string(), LifecycleCommandSingle::Exec(vec![]));
        map.insert(
            "real".to_string(),
            LifecycleCommandSingle::Exec(vec!["echo".to_string()]),
        );
        let cmd = LifecycleCommand::Parallel(map);
        assert_eq!(cmd.argvs(), vec![vec!["echo".to_string()]]);
    }

    // --- LifecycleCommand::substitute ---

    fn upcase(s: &str) -> String {
        s.to_uppercase()
    }

    #[test]
    fn substitute_shell() {
        let cmd = LifecycleCommand::Shell("hello".to_string());
        assert_eq!(
            cmd.substitute(&upcase),
            LifecycleCommand::Shell("HELLO".to_string())
        );
    }

    #[test]
    fn substitute_exec() {
        let cmd = LifecycleCommand::Exec(vec!["echo".to_string(), "hi".to_string()]);
        assert_eq!(
            cmd.substitute(&upcase),
            LifecycleCommand::Exec(vec!["ECHO".to_string(), "HI".to_string()])
        );
    }

    #[test]
    fn substitute_parallel() {
        let mut map = IndexMap::new();
        map.insert(
            "a".to_string(),
            LifecycleCommandSingle::Shell("hi".to_string()),
        );
        let cmd = LifecycleCommand::Parallel(map);
        match cmd.substitute(&upcase) {
            LifecycleCommand::Parallel(map) => {
                assert_eq!(
                    map.get("a"),
                    Some(&LifecycleCommandSingle::Shell("HI".to_string()))
                );
            }
            other => panic!("expected Parallel, got {other:?}"),
        }
    }

    // --- LifecycleCommand::try_substitute ---

    #[test]
    fn try_substitute_maps_each_leaf() {
        let cmd = LifecycleCommand::Exec(vec!["echo".to_string(), "hi".to_string()]);
        let out = cmd.try_substitute(&|s: &str| Ok(s.to_uppercase())).unwrap();
        assert_eq!(
            out,
            LifecycleCommand::Exec(vec!["ECHO".to_string(), "HI".to_string()])
        );
    }

    #[test]
    fn try_substitute_propagates_first_error() {
        let mut map = IndexMap::new();
        map.insert(
            "a".to_string(),
            LifecycleCommandSingle::Shell("ok".to_string()),
        );
        map.insert(
            "b".to_string(),
            LifecycleCommandSingle::Shell("boom".to_string()),
        );
        let cmd = LifecycleCommand::Parallel(map);
        let err = cmd
            .try_substitute(&|s: &str| {
                if s == "boom" {
                    anyhow::bail!("bad leaf: {s}")
                } else {
                    Ok(s.to_string())
                }
            })
            .unwrap_err();
        assert!(err.to_string().contains("bad leaf: boom"), "got: {err}");
    }

    // --- LifecycleHooks ---

    #[test]
    fn lifecycle_hooks_deserialize_camel_case() {
        let hooks: LifecycleHooks = serde_json::from_str(
            r#"{"onCreateCommand": "echo create", "postStartCommand": ["echo", "start"]}"#,
        )
        .unwrap();
        assert_eq!(
            hooks.on_create_command,
            Some(LifecycleCommand::Shell("echo create".to_string()))
        );
        assert_eq!(
            hooks.post_start_command,
            Some(LifecycleCommand::Exec(vec![
                "echo".to_string(),
                "start".to_string()
            ]))
        );
        assert!(hooks.update_content_command.is_none());
        assert!(hooks.post_create_command.is_none());
        assert!(hooks.post_attach_command.is_none());
    }

    #[test]
    fn lifecycle_hooks_default_is_empty() {
        assert!(LifecycleHooks::default().is_empty());
    }

    #[test]
    fn lifecycle_hooks_with_one_field_is_not_empty() {
        let hooks = LifecycleHooks {
            on_create_command: Some(LifecycleCommand::Shell("echo".to_string())),
            ..Default::default()
        };
        assert!(!hooks.is_empty());
    }

    #[test]
    fn lifecycle_hooks_substitute_maps_all_fields() {
        let hooks = LifecycleHooks {
            on_create_command: Some(LifecycleCommand::Shell("a".to_string())),
            update_content_command: Some(LifecycleCommand::Shell("b".to_string())),
            post_create_command: Some(LifecycleCommand::Shell("c".to_string())),
            post_start_command: Some(LifecycleCommand::Shell("d".to_string())),
            post_attach_command: Some(LifecycleCommand::Shell("e".to_string())),
        };
        let result = hooks.substitute(&upcase);
        assert_eq!(
            result.on_create_command,
            Some(LifecycleCommand::Shell("A".to_string()))
        );
        assert_eq!(
            result.update_content_command,
            Some(LifecycleCommand::Shell("B".to_string()))
        );
        assert_eq!(
            result.post_create_command,
            Some(LifecycleCommand::Shell("C".to_string()))
        );
        assert_eq!(
            result.post_start_command,
            Some(LifecycleCommand::Shell("D".to_string()))
        );
        assert_eq!(
            result.post_attach_command,
            Some(LifecycleCommand::Shell("E".to_string()))
        );
    }

    // --- HOOKS ordering ---

    #[test]
    fn hooks_const_order_matches_spec() {
        let names: Vec<&str> = HOOKS.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            names,
            vec![
                "onCreateCommand",
                "updateContentCommand",
                "postCreateCommand",
                "postStartCommand",
                "postAttachCommand",
            ]
        );
    }

    #[test]
    fn hooks_const_accessors_match_fields() {
        let hooks = LifecycleHooks {
            on_create_command: Some(LifecycleCommand::Shell("on-create".to_string())),
            update_content_command: Some(LifecycleCommand::Shell("update-content".to_string())),
            post_create_command: Some(LifecycleCommand::Shell("post-create".to_string())),
            post_start_command: Some(LifecycleCommand::Shell("post-start".to_string())),
            post_attach_command: Some(LifecycleCommand::Shell("post-attach".to_string())),
        };
        let values: Vec<Option<&LifecycleCommand>> =
            HOOKS.iter().map(|(_, get)| get(&hooks).as_ref()).collect();
        assert_eq!(
            values,
            vec![
                Some(&LifecycleCommand::Shell("on-create".to_string())),
                Some(&LifecycleCommand::Shell("update-content".to_string())),
                Some(&LifecycleCommand::Shell("post-create".to_string())),
                Some(&LifecycleCommand::Shell("post-start".to_string())),
                Some(&LifecycleCommand::Shell("post-attach".to_string())),
            ]
        );
    }

    // --- run_on_host (no Docker required) ---

    #[tokio::test]
    async fn run_on_host_shell_success() {
        let tmp = tempfile::tempdir().unwrap();
        let cmd = LifecycleCommand::Shell("exit 0".to_string());
        run_on_host(&cmd, tmp.path()).await.unwrap();
    }

    #[tokio::test]
    async fn run_on_host_shell_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let cmd = LifecycleCommand::Shell("exit 7".to_string());
        let err = run_on_host(&cmd, tmp.path()).await.unwrap_err();
        assert!(
            err.to_string().contains('7'),
            "expected exit code 7 in error, got: {err}"
        );
    }

    #[tokio::test]
    async fn run_on_host_runs_in_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        let marker = tmp.path().join("marker");
        let cmd = LifecycleCommand::Exec(vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "pwd > marker".to_string(),
        ]);
        run_on_host(&cmd, tmp.path()).await.unwrap();
        let contents = std::fs::read_to_string(marker).unwrap();
        assert_eq!(contents.trim(), tmp.path().to_str().unwrap());
    }

    #[tokio::test]
    async fn run_on_host_parallel_all_succeed() {
        let tmp = tempfile::tempdir().unwrap();
        let mut map = IndexMap::new();
        map.insert(
            "a".to_string(),
            LifecycleCommandSingle::Exec(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "echo a > a.txt".to_string(),
            ]),
        );
        map.insert(
            "b".to_string(),
            LifecycleCommandSingle::Exec(vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                "echo b > b.txt".to_string(),
            ]),
        );
        let cmd = LifecycleCommand::Parallel(map);
        run_on_host(&cmd, tmp.path()).await.unwrap();
        assert!(tmp.path().join("a.txt").exists());
        assert!(tmp.path().join("b.txt").exists());
    }

    #[tokio::test]
    async fn run_on_host_parallel_propagates_failure() {
        let tmp = tempfile::tempdir().unwrap();
        let mut map = IndexMap::new();
        map.insert(
            "ok".to_string(),
            LifecycleCommandSingle::Shell("exit 0".to_string()),
        );
        map.insert(
            "fail".to_string(),
            LifecycleCommandSingle::Shell("exit 3".to_string()),
        );
        let cmd = LifecycleCommand::Parallel(map);
        let err = run_on_host(&cmd, tmp.path()).await.unwrap_err();
        assert!(
            err.to_string().contains('3'),
            "expected exit code 3 in error, got: {err}"
        );
    }

    #[tokio::test]
    async fn run_on_host_empty_command_is_noop() {
        let tmp = tempfile::tempdir().unwrap();
        let cmd = LifecycleCommand::Exec(vec![]);
        run_on_host(&cmd, tmp.path()).await.unwrap();
    }
}
