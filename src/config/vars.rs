use std::collections::HashMap;

use crate::{cache::CacheDir, config::DevcontainerConfig, workspace::Workspace};

pub(crate) const CONTAINER_WORKSPACE: &str = "/workspace";
pub(crate) const CONTAINER_CACHE: &str = "/cache";

/// Host-environment lookup used to resolve `${localEnv:VAR}`: maps a variable
/// name to its value, or `None` when the variable is unset.
type LocalEnvLookup<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Applies variable substitution to container_env, remote_env, and mounts strings.
pub(crate) fn apply_substitutions(
    config: DevcontainerConfig,
    workspace: &Workspace,
    cache_dir: &CacheDir,
) -> DevcontainerConfig {
    let local_workspace = workspace.root.to_string_lossy().into_owned();
    let local_cache = cache_dir.host_path.to_string_lossy().into_owned();

    DevcontainerConfig {
        image: config.image,
        features: config.features,
        container_env: config
            .container_env
            .into_iter()
            .map(|(k, v)| (k, apply_container_env_substitution(&v)))
            .collect(),
        remote_env: config
            .remote_env
            .into_iter()
            .map(|(k, v)| (k, apply_substitution(&v, &local_workspace, &local_cache)))
            .collect(),
        container_user: config.container_user,
        mounts: config
            .mounts
            .into_iter()
            .map(|m| apply_substitution(&m, &local_workspace, &local_cache))
            .collect(),
        forward_ports: config.forward_ports,
        initialize_command: config.initialize_command.as_ref().map(|c| {
            c.substitute(&|s: &str| apply_substitution(s, &local_workspace, &local_cache))
        }),
        lifecycle: config
            .lifecycle
            .substitute(&|s: &str| apply_substitution(s, &local_workspace, &local_cache)),
        scripts: config.scripts,
    }
}

/// Substitutes all variable occurrences in `s`, including `${localEnv:VAR}` read
/// from the host environment. Emits tracing::warn! for unknowns. Used for the
/// runtime-applied fields (remoteEnv, mounts, lifecycle commands, and the
/// container command), for both devcontainer.json and feature contributions.
pub(crate) fn apply_substitution(s: &str, local_workspace: &str, local_cache: &str) -> String {
    apply_to_string(
        s,
        Some(local_workspace),
        Some(local_cache),
        Some(&|name| std::env::var(name).ok()),
    )
}

/// Applies only container-side variable substitution (no local variables, no
/// `localEnv`). Used for `containerEnv` values in both devcontainer.json and
/// feature metadata, which are baked into the image at build time.
pub(crate) fn apply_container_env_substitution(s: &str) -> String {
    apply_to_string(s, None, None, None)
}

/// Returns every `${...}` token still present in `s`, in order of appearance.
///
/// Once [`apply_substitution`] has run, all four supported variables are already
/// replaced, so any token returned here is an unresolved reference — either a
/// completely unknown variable (e.g. `${localEnv:HOME}`) or a known one used in
/// a context where it is unavailable. Malformed `${...` with no closing brace is
/// not treated as a token, matching [`substitute`].
pub(crate) fn unresolved_variables(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < s.len() {
        if bytes[i..].starts_with(b"${") {
            if let Some(end_offset) = s[i + 2..].find('}') {
                tokens.push(s[i..i + 2 + end_offset + 1].to_string());
                i += 2 + end_offset + 1;
            } else {
                // No closing '}' — not a variable; skip the '$' and keep scanning.
                i += 1;
            }
        } else {
            let ch = s[i..]
                .chars()
                .next()
                .expect("i < s.len() guaranteed by while condition");
            i += ch.len_utf8();
        }
    }
    tokens
}

/// Resolves `${containerEnv:NAME}` / `${containerEnv:NAME:default}` references in
/// `s` against `env` (the image's baked environment plus the container user's runtime
/// HOME/USER, from `exec.rs`). A name that is absent from `env` *or* maps to an empty
/// value falls back to the `:default` text if one is given (an explicit empty default,
/// `${containerEnv:NAME:}`, opts into an empty value); otherwise it is an error, so a
/// typo or unsupported variable fails loudly instead of silently becoming empty. All
/// other text, including non-`containerEnv` `${…}` tokens, is copied through unchanged.
/// Run at run time, after host/`localEnv` substitution has already deferred these tokens.
pub(crate) fn resolve_container_env(
    s: &str,
    env: &HashMap<String, String>,
) -> anyhow::Result<String> {
    let mut result = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < s.len() {
        if bytes[i..].starts_with(b"${") {
            if let Some(end_offset) = s[i + 2..].find('}') {
                let name = &s[i + 2..i + 2 + end_offset];
                let token = &s[i..i + 2 + end_offset + 1];
                i += 2 + end_offset + 1;
                if let Some(rest) = name.strip_prefix("containerEnv:") {
                    let (var, default) = match rest.split_once(':') {
                        Some((var, default)) => (var, Some(default)),
                        None => (rest, None),
                    };
                    // Treat an empty value as unset, so it falls back to a default or errors.
                    let value = match env.get(var) {
                        Some(v) if !v.is_empty() => v.clone(),
                        _ => match default {
                            Some(d) => d.to_owned(),
                            None => anyhow::bail!(
                                "containerEnv variable `{var}` is undefined or empty; dcc \
                                 resolves ${{containerEnv:VAR}} against the image environment \
                                 plus the container user's HOME and USER. Add a default (e.g. \
                                 ${{containerEnv:{var}:/fallback}}) to allow an empty or \
                                 fallback value."
                            ),
                        },
                    };
                    result.push_str(&value);
                } else {
                    result.push_str(token);
                }
                continue;
            }
            // No closing '}' — not a variable; copy '$' literally.
            result.push('$');
            i += 1;
        } else {
            let ch = s[i..]
                .chars()
                .next()
                .expect("i < s.len() guaranteed by while condition");
            result.push(ch);
            i += ch.len_utf8();
        }
    }
    Ok(result)
}

fn apply_to_string(
    s: &str,
    local_workspace: Option<&str>,
    local_cache: Option<&str>,
    local_env: Option<LocalEnvLookup<'_>>,
) -> String {
    let (result, unknowns) = substitute(s, local_workspace, local_cache, local_env);
    for u in unknowns {
        tracing::warn!(variable = %u, "unknown variable reference left as-is");
    }
    result
}

/// Resolves a `${localEnv:VAR}` or `${localEnv:VAR:default}` reference.
///
/// Returns `None` when `name` is not a `localEnv:` reference or when no host-env
/// `lookup` is provided (so the caller leaves the token literal). When the host
/// variable is unset, the `:default` text is used if present, otherwise the empty
/// string (matching the devcontainer spec).
fn resolve_local_env(name: &str, lookup: Option<LocalEnvLookup<'_>>) -> Option<String> {
    let rest = name.strip_prefix("localEnv:")?;
    let lookup = lookup?;
    let (var, default) = match rest.split_once(':') {
        Some((var, default)) => (var, Some(default)),
        None => (rest, None),
    };
    Some(
        lookup(var)
            .or_else(|| default.map(str::to_owned))
            .unwrap_or_default(),
    )
}

/// Pure substitution: returns (substituted_string, list_of_unknown_variable_names).
/// Unknowns are returned rather than warned here so the function is testable without tracing.
/// `local_env`, when `Some`, enables `${localEnv:VAR}` resolution via the given host-env lookup.
fn substitute(
    s: &str,
    local_workspace: Option<&str>,
    local_cache: Option<&str>,
    local_env: Option<LocalEnvLookup<'_>>,
) -> (String, Vec<String>) {
    let mut result = String::with_capacity(s.len());
    let mut unknowns = Vec::new();
    let mut i = 0;
    let bytes = s.as_bytes();

    while i < s.len() {
        if bytes[i..].starts_with(b"${") {
            // Find closing '}'
            if let Some(end_offset) = s[i + 2..].find('}') {
                let name = &s[i + 2..i + 2 + end_offset];
                let token = &s[i..i + 2 + end_offset + 1];
                i += 2 + end_offset + 1;
                if name.starts_with("containerEnv:") {
                    // Deferred: resolved at run time by `resolve_container_env`
                    // against the image's Config.Env. Left intact here (not an
                    // unknown), so it neither warns nor trips `unresolved_variables`.
                    result.push_str(token);
                    continue;
                }
                let resolved: Option<String> = match name {
                    "localCacheFolder" => local_cache.map(str::to_owned),
                    "containerCacheFolder" => Some(CONTAINER_CACHE.to_owned()),
                    "localWorkspaceFolder" => local_workspace.map(str::to_owned),
                    "containerWorkspaceFolder" => Some(CONTAINER_WORKSPACE.to_owned()),
                    _ => resolve_local_env(name, local_env),
                };
                match resolved {
                    Some(r) => result.push_str(&r),
                    None => {
                        // Unknown name, or a known one unavailable in this context — leave as-is.
                        unknowns.push(token.to_owned());
                        result.push_str(token);
                    }
                }
            } else {
                // No closing '}' — not a variable, copy '$' literally
                result.push('$');
                i += 1;
            }
        } else {
            let ch = s[i..]
                .chars()
                .next()
                .expect("i < s.len() guaranteed by while condition");
            result.push(ch);
            i += ch.len_utf8();
        }
    }

    (result, unknowns)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sub(s: &str, ws: &str, cache: &str) -> String {
        let (r, _) = substitute(s, Some(ws), Some(cache), None);
        r
    }

    fn unknowns(s: &str) -> Vec<String> {
        let (_, u) = substitute(s, Some("/ws"), Some("/cache"), None);
        u
    }

    fn sub_env(s: &str, lookup: LocalEnvLookup<'_>) -> String {
        let (r, _) = substitute(s, Some("/ws"), Some("/c"), Some(lookup));
        r
    }

    #[test]
    fn sub_local_cache_folder() {
        assert_eq!(sub("${localCacheFolder}/.cargo", "/ws", "/c"), "/c/.cargo");
    }

    #[test]
    fn sub_container_cache_folder() {
        assert_eq!(
            sub("${containerCacheFolder}/.cargo", "/ws", "/c"),
            "/cache/.cargo"
        );
    }

    #[test]
    fn sub_local_workspace_folder() {
        assert_eq!(
            sub("${localWorkspaceFolder}/src", "/project", "/c"),
            "/project/src"
        );
    }

    #[test]
    fn sub_container_workspace_folder() {
        assert_eq!(
            sub("${containerWorkspaceFolder}/target", "/ws", "/c"),
            "/workspace/target"
        );
    }

    #[test]
    fn sub_multiple_variables() {
        let s = "type=bind,src=${localCacheFolder}/target,dst=${containerWorkspaceFolder}/target";
        assert_eq!(
            sub(s, "/ws", "/c"),
            "type=bind,src=/c/target,dst=/workspace/target"
        );
    }

    #[test]
    fn sub_repeated_variable() {
        assert_eq!(
            sub("${localCacheFolder}:${localCacheFolder}", "/ws", "/c"),
            "/c:/c"
        );
    }

    #[test]
    fn sub_no_variables() {
        assert_eq!(sub("plain string", "/ws", "/c"), "plain string");
    }

    #[test]
    fn sub_empty_string() {
        assert_eq!(sub("", "/ws", "/c"), "");
    }

    #[test]
    fn unknown_variable_passthrough() {
        assert_eq!(sub("${unknownVar}", "/ws", "/c"), "${unknownVar}");
        assert_eq!(unknowns("${unknownVar}"), vec!["${unknownVar}"]);
    }

    #[test]
    fn unknown_mixed_with_known() {
        assert_eq!(
            sub("${localCacheFolder}:${UNKNOWN}", "/c", "/c"),
            "/c:${UNKNOWN}"
        );
        assert_eq!(unknowns("${UNKNOWN}"), vec!["${UNKNOWN}"]);
    }

    #[test]
    fn malformed_no_closing_brace() {
        // ${noClose is not a variable — the '$' is kept literally
        assert_eq!(sub("${noClose", "/ws", "/c"), "${noClose");
        assert_eq!(unknowns("${noClose"), vec![] as Vec<String>);
    }

    #[test]
    fn all_four_variables_in_one_string() {
        let s = "${localCacheFolder} ${containerCacheFolder} ${localWorkspaceFolder} ${containerWorkspaceFolder}";
        assert_eq!(sub(s, "/ws", "/lc"), "/lc /cache /ws /workspace");
    }

    #[test]
    fn local_var_in_container_env_context_is_unknown() {
        // When local vars are None (container-only context), local vars are left as-is
        let result = apply_container_env_substitution("${localCacheFolder}/x");
        assert_eq!(result, "${localCacheFolder}/x");
    }

    #[test]
    fn container_var_in_container_env_context_is_substituted() {
        let result = apply_container_env_substitution("${containerCacheFolder}/x");
        assert_eq!(result, "/cache/x");
    }

    // --- localEnv ---

    #[test]
    fn localenv_resolves_from_host() {
        let look = |n: &str| {
            if n == "HOME" {
                Some("/home/me".to_owned())
            } else {
                None
            }
        };
        assert_eq!(
            sub_env("${localEnv:HOME}/.gitconfig", &look),
            "/home/me/.gitconfig"
        );
    }

    #[test]
    fn localenv_undefined_resolves_to_empty() {
        let look = |_: &str| None;
        assert_eq!(sub_env("x${localEnv:MISSING}y", &look), "xy");
    }

    #[test]
    fn localenv_default_used_when_unset() {
        let look = |_: &str| None;
        assert_eq!(
            sub_env("${localEnv:MISSING:/tmp/fallback}", &look),
            "/tmp/fallback"
        );
    }

    #[test]
    fn localenv_default_ignored_when_set() {
        let look = |n: &str| {
            if n == "VAR" {
                Some("real".to_owned())
            } else {
                None
            }
        };
        assert_eq!(sub_env("${localEnv:VAR:fallback}", &look), "real");
    }

    #[test]
    fn localenv_default_preserves_colons() {
        let look = |_: &str| None;
        assert_eq!(sub_env("${localEnv:X:a:b:c}", &look), "a:b:c");
    }

    #[test]
    fn localenv_left_literal_when_lookup_absent() {
        // local_env = None (e.g. containerEnv) → token unresolved and reported.
        let (r, u) = substitute("${localEnv:HOME}", Some("/ws"), Some("/c"), None);
        assert_eq!(r, "${localEnv:HOME}");
        assert_eq!(u, vec!["${localEnv:HOME}".to_owned()]);
    }

    #[test]
    fn localenv_not_substituted_in_container_env_context() {
        assert_eq!(
            apply_container_env_substitution("${localEnv:HOME}"),
            "${localEnv:HOME}"
        );
    }

    #[test]
    fn localenv_mixed_with_folder_vars() {
        let look = |n: &str| {
            if n == "USER" {
                Some("dev".to_owned())
            } else {
                None
            }
        };
        assert_eq!(
            sub_env("${localWorkspaceFolder}:${localEnv:USER}", &look),
            "/ws:dev"
        );
    }

    // --- containerEnv (resolve_container_env + deferral) ---

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn container_env_resolves_from_map() {
        let e = env(&[("PATH", "/usr/bin:/bin")]);
        assert_eq!(
            resolve_container_env("${containerEnv:PATH}:/x", &e).unwrap(),
            "/usr/bin:/bin:/x"
        );
    }

    #[test]
    fn container_env_unset_errors() {
        let err = resolve_container_env("a${containerEnv:MISSING}b", &env(&[])).unwrap_err();
        assert!(
            err.to_string().contains("MISSING"),
            "error should name the variable, got: {err}"
        );
    }

    #[test]
    fn container_env_empty_value_errors() {
        // A present-but-empty value is treated as unset.
        let err = resolve_container_env("${containerEnv:X}", &env(&[("X", "")])).unwrap_err();
        assert!(err.to_string().contains('X'), "got: {err}");
    }

    #[test]
    fn container_env_default_used_when_unset() {
        assert_eq!(
            resolve_container_env("${containerEnv:MISSING:/fallback}", &env(&[])).unwrap(),
            "/fallback"
        );
    }

    #[test]
    fn container_env_default_used_when_empty() {
        assert_eq!(
            resolve_container_env("${containerEnv:X:/fallback}", &env(&[("X", "")])).unwrap(),
            "/fallback"
        );
    }

    #[test]
    fn container_env_explicit_empty_default_allows_empty() {
        assert_eq!(
            resolve_container_env("a${containerEnv:MISSING:}b", &env(&[])).unwrap(),
            "ab"
        );
    }

    #[test]
    fn container_env_default_ignored_when_set() {
        let e = env(&[("X", "real")]);
        assert_eq!(
            resolve_container_env("${containerEnv:X:fallback}", &e).unwrap(),
            "real"
        );
    }

    #[test]
    fn container_env_default_preserves_colons() {
        assert_eq!(
            resolve_container_env("${containerEnv:X:a:b:c}", &env(&[])).unwrap(),
            "a:b:c"
        );
    }

    #[test]
    fn container_env_leaves_other_tokens_untouched() {
        let e = env(&[("PATH", "/p")]);
        assert_eq!(
            resolve_container_env(
                "${localEnv:HOME}|${localWorkspaceFolder}|${unknown}|${containerEnv:PATH}",
                &e
            )
            .unwrap(),
            "${localEnv:HOME}|${localWorkspaceFolder}|${unknown}|/p"
        );
    }

    #[test]
    fn container_env_malformed_no_closing_brace_left_literal() {
        assert_eq!(
            resolve_container_env("${containerEnv:X", &env(&[("X", "v")])).unwrap(),
            "${containerEnv:X"
        );
    }

    #[test]
    fn substitute_defers_container_env_without_warning() {
        // The host pass leaves containerEnv tokens intact and does NOT mark them unknown.
        let (r, u) = substitute("${containerEnv:PATH}:/x", Some("/ws"), Some("/c"), None);
        assert_eq!(r, "${containerEnv:PATH}:/x");
        assert!(
            u.is_empty(),
            "containerEnv should be deferred, got unknowns: {u:?}"
        );
    }

    // --- unresolved_variables ---

    #[test]
    fn unresolved_finds_localenv_in_mount_string() {
        let mount = "type=bind,source=${localEnv:HOME}/.gitconfig,target=/run/host-gitconfig";
        assert_eq!(
            unresolved_variables(mount),
            vec!["${localEnv:HOME}".to_string()]
        );
    }

    #[test]
    fn unresolved_empty_after_supported_vars_substituted() {
        let substituted = sub(
            "${localWorkspaceFolder}/a:${localCacheFolder}/b",
            "/ws",
            "/c",
        );
        assert!(unresolved_variables(&substituted).is_empty());
    }

    #[test]
    fn unresolved_collects_multiple_tokens_in_order() {
        assert_eq!(
            unresolved_variables("${localEnv:A}-${localEnv:B}"),
            vec!["${localEnv:A}".to_string(), "${localEnv:B}".to_string()]
        );
    }

    #[test]
    fn unresolved_none_when_no_tokens() {
        assert!(unresolved_variables("type=bind,source=/plain/path,target=/x").is_empty());
    }

    #[test]
    fn unresolved_ignores_malformed_no_closing_brace() {
        assert!(unresolved_variables("${noClose/path").is_empty());
    }
}
