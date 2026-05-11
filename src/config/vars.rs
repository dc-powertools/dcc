use crate::{cache::CacheDir, config::DevcontainerConfig, workspace::Workspace};

pub(crate) const CONTAINER_WORKSPACE: &str = "/workspace";
pub(crate) const CONTAINER_CACHE: &str = "/cache";

/// Applies variable substitution to container_env values and mounts strings.
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
            .map(|(k, v)| (k, apply_to_string(&v, &local_workspace, &local_cache)))
            .collect(),
        container_user: config.container_user,
        mounts: config
            .mounts
            .into_iter()
            .map(|m| apply_to_string(&m, &local_workspace, &local_cache))
            .collect(),
        forward_ports: config.forward_ports,
        entrypoint: config.entrypoint,
    }
}

/// Substitutes all variable occurrences in `s`. Emits tracing::warn! for unknowns.
fn apply_to_string(s: &str, local_workspace: &str, local_cache: &str) -> String {
    let (result, unknowns) = substitute(s, local_workspace, local_cache);
    for u in unknowns {
        tracing::warn!(variable = %u, "unknown variable reference left as-is");
    }
    result
}

/// Pure substitution: returns (substituted_string, list_of_unknown_variable_names).
/// Unknowns are returned rather than warned here so the function is testable without tracing.
fn substitute(s: &str, local_workspace: &str, local_cache: &str) -> (String, Vec<String>) {
    let mut result = String::with_capacity(s.len());
    let mut unknowns = Vec::new();
    let mut i = 0;
    let bytes = s.as_bytes();

    while i < s.len() {
        if bytes[i..].starts_with(b"${") {
            // Find closing '}'
            if let Some(end_offset) = s[i + 2..].find('}') {
                let name = &s[i + 2..i + 2 + end_offset];
                let replacement = match name {
                    "localCacheFolder" => Some(local_cache),
                    "containerCacheFolder" => Some(CONTAINER_CACHE),
                    "localWorkspaceFolder" => Some(local_workspace),
                    "containerWorkspaceFolder" => Some(CONTAINER_WORKSPACE),
                    _ => None,
                };
                if let Some(r) = replacement {
                    result.push_str(r);
                } else {
                    unknowns.push(format!("${{{name}}}"));
                    result.push_str(&s[i..i + 2 + end_offset + 1]);
                }
                i += 2 + end_offset + 1;
            } else {
                // No closing '}' — not a variable, copy '$' literally
                result.push('$');
                i += 1;
            }
        } else {
            // Safe: we're iterating ASCII-by-ASCII through UTF-8 string
            // For non-ASCII, copy the full char
            let ch = s[i..].chars().next().unwrap();
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
        let (r, _) = substitute(s, ws, cache);
        r
    }

    fn unknowns(s: &str) -> Vec<String> {
        let (_, u) = substitute(s, "/ws", "/cache");
        u
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
}
