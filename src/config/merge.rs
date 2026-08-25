use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;

use crate::config::{Customizations, RawConfig, RawDccConfig};

pub(crate) fn merge(parent: RawConfig, child: RawConfig) -> RawConfig {
    RawConfig {
        extends: None,
        name: child.name.or(parent.name),
        image: child.image.or(parent.image),
        build: child.build.or(parent.build),
        features: merge_option_index_maps(parent.features, child.features),
        container_env: merge_option_hash_maps(parent.container_env, child.container_env),
        remote_env: merge_option_hash_maps(parent.remote_env, child.remote_env),
        container_user: child.container_user.or(parent.container_user),
        mounts: merge_option_vecs(parent.mounts, child.mounts),
        run_args: merge_option_vecs(parent.run_args, child.run_args),
        privileged: child.privileged.or(parent.privileged),
        cap_add: merge_option_vecs(parent.cap_add, child.cap_add),
        security_opt: merge_option_vecs(parent.security_opt, child.security_opt),
        forward_ports: merge_option_vecs(parent.forward_ports, child.forward_ports),
        ports_attributes: merge_option_hash_maps(parent.ports_attributes, child.ports_attributes),
        other_ports_attributes: child
            .other_ports_attributes
            .or(parent.other_ports_attributes),
        override_command: child.override_command.or(parent.override_command),
        update_remote_user_uid: child
            .update_remote_user_uid
            .or(parent.update_remote_user_uid),
        workspace_folder: child.workspace_folder.or(parent.workspace_folder),
        workspace_mount: child.workspace_mount.or(parent.workspace_mount),
        initialize_command: child.initialize_command.or(parent.initialize_command),
        on_create_command: child.on_create_command.or(parent.on_create_command),
        update_content_command: child
            .update_content_command
            .or(parent.update_content_command),
        post_create_command: child.post_create_command.or(parent.post_create_command),
        post_start_command: child.post_start_command.or(parent.post_start_command),
        post_attach_command: child.post_attach_command.or(parent.post_attach_command),
        scripts: merge_option_hash_maps(parent.scripts, child.scripts),
        customizations: merge_customizations(parent.customizations, child.customizations),
        extra: merge_hash_maps(parent.extra, child.extra),
    }
}

fn merge_customizations(
    parent: Option<Customizations>,
    child: Option<Customizations>,
) -> Option<Customizations> {
    match (parent, child) {
        (None, None) => None,
        (p, None) => p,
        (None, c) => c,
        (Some(p), Some(c)) => Some(Customizations {
            dcc: merge_dcc(p.dcc, c.dcc),
            other: merge_hash_maps(p.other, c.other),
        }),
    }
}

fn merge_dcc(parent: Option<RawDccConfig>, child: Option<RawDccConfig>) -> Option<RawDccConfig> {
    match (parent, child) {
        (None, None) => None,
        (p, None) => p,
        (None, c) => c,
        (Some(p), Some(c)) => Some(RawDccConfig {
            extends: None,
            commands: merge_option_hash_maps(p.commands, c.commands),
            state: merge_option_vecs(p.state, c.state),
            registry_cas: match (p.registry_cas, c.registry_cas) {
                (None, None) => None,
                (parent, None) => parent,
                (None, child) => child,
                (Some(parent), Some(child)) => Some(parent.merge(child)),
            },
            extra: merge_hash_maps(p.extra, c.extra),
        }),
    }
}

fn merge_option_index_maps<V: Clone>(
    parent: Option<IndexMap<String, V>>,
    child: Option<IndexMap<String, V>>,
) -> Option<IndexMap<String, V>> {
    match (parent, child) {
        (None, None) => None,
        (p, None) => p,
        (None, c) => c,
        (Some(mut p), Some(c)) => {
            // Child value wins on conflict, but parent keys keep their position.
            // IndexMap::insert replaces the value for an existing key while preserving
            // the key's insertion order, so parent keys stay at their original positions.
            for (k, v) in c {
                p.insert(k, v);
            }
            Some(p)
        }
    }
}

fn merge_option_hash_maps<V>(
    parent: Option<HashMap<String, V>>,
    child: Option<HashMap<String, V>>,
) -> Option<HashMap<String, V>> {
    match (parent, child) {
        (None, None) => None,
        (p, None) => p,
        (None, c) => c,
        (Some(mut p), Some(c)) => {
            p.extend(c); // child wins on conflict
            Some(p)
        }
    }
}

fn merge_option_vecs<T: Eq + std::hash::Hash + Clone>(
    parent: Option<Vec<T>>,
    child: Option<Vec<T>>,
) -> Option<Vec<T>> {
    match (parent, child) {
        (None, None) => None,
        (p, None) => p,
        (None, c) => c,
        (Some(p), Some(c)) => {
            let mut seen: HashSet<T> = HashSet::new();
            let mut result = Vec::with_capacity(p.len() + c.len());
            for item in p.into_iter().chain(c) {
                if seen.insert(item.clone()) {
                    result.push(item);
                }
            }
            Some(result)
        }
    }
}

fn merge_hash_maps<V>(
    mut parent: HashMap<String, V>,
    child: HashMap<String, V>,
) -> HashMap<String, V> {
    parent.extend(child);
    parent
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn empty() -> RawConfig {
        RawConfig {
            extends: None,
            name: None,
            image: None,
            build: None,
            features: None,
            container_env: None,
            remote_env: None,
            container_user: None,
            mounts: None,
            run_args: None,
            privileged: None,
            cap_add: None,
            security_opt: None,
            forward_ports: None,
            ports_attributes: None,
            other_ports_attributes: None,
            override_command: None,
            update_remote_user_uid: None,
            workspace_folder: None,
            workspace_mount: None,
            initialize_command: None,
            on_create_command: None,
            update_content_command: None,
            post_create_command: None,
            post_start_command: None,
            post_attach_command: None,
            scripts: None,
            customizations: None,
            extra: Default::default(),
        }
    }

    #[test]
    fn extends_always_none() {
        let parent = RawConfig {
            extends: Some("parent-base.json".to_string()),
            ..empty()
        };
        let child = RawConfig {
            extends: Some("child-base.json".to_string()),
            ..empty()
        };
        let result = merge(parent, child);
        assert!(result.extends.is_none());
    }

    #[test]
    fn name_child_wins() {
        let parent = RawConfig {
            name: Some("parent".to_string()),
            ..empty()
        };
        let child = RawConfig {
            name: Some("child".to_string()),
            ..empty()
        };
        let result = merge(parent, child);
        assert_eq!(result.name.as_deref(), Some("child"));
    }

    #[test]
    fn name_child_none_uses_parent() {
        let parent = RawConfig {
            name: Some("parent".to_string()),
            ..empty()
        };
        let child = empty();
        let result = merge(parent, child);
        assert_eq!(result.name.as_deref(), Some("parent"));
    }

    #[test]
    fn name_parent_none_uses_child() {
        let parent = empty();
        let child = RawConfig {
            name: Some("child".to_string()),
            ..empty()
        };
        let result = merge(parent, child);
        assert_eq!(result.name.as_deref(), Some("child"));
    }

    #[test]
    fn name_both_none_stays_none() {
        let result = merge(empty(), empty());
        assert!(result.name.is_none());
    }

    #[test]
    fn image_child_wins() {
        let parent = RawConfig {
            image: Some("p:1".to_string()),
            ..empty()
        };
        let child = RawConfig {
            image: Some("c:2".to_string()),
            ..empty()
        };
        let result = merge(parent, child);
        assert_eq!(result.image.as_deref(), Some("c:2"));
    }

    #[test]
    fn image_child_none_uses_parent() {
        let parent = RawConfig {
            image: Some("p:1".to_string()),
            ..empty()
        };
        let child = empty();
        let result = merge(parent, child);
        assert_eq!(result.image.as_deref(), Some("p:1"));
    }

    #[test]
    fn build_child_wins() {
        let parent = RawConfig {
            build: Some(crate::config::BuildConfig {
                context: "parent".to_string(),
                dockerfile: "Dockerfile.parent".to_string(),
                args: HashMap::new(),
                target: None,
            }),
            ..empty()
        };
        let child = RawConfig {
            build: Some(crate::config::BuildConfig {
                context: "child".to_string(),
                dockerfile: "Dockerfile.child".to_string(),
                args: HashMap::new(),
                target: Some("dev".to_string()),
            }),
            ..empty()
        };
        let result = merge(parent, child);
        let build = result.build.expect("build should be present");
        assert_eq!(build.context, "child");
        assert_eq!(build.dockerfile, "Dockerfile.child");
        assert_eq!(build.target.as_deref(), Some("dev"));
    }

    #[test]
    fn features_union_no_conflict() {
        let mut parent_features = IndexMap::new();
        parent_features.insert(
            "a".to_string(),
            serde_json::Value::Object(serde_json::Map::new()),
        );
        let mut child_features = IndexMap::new();
        child_features.insert(
            "b".to_string(),
            serde_json::Value::Object(serde_json::Map::new()),
        );
        let parent = RawConfig {
            features: Some(parent_features),
            ..empty()
        };
        let child = RawConfig {
            features: Some(child_features),
            ..empty()
        };
        let result = merge(parent, child);
        let features = result.features.unwrap();
        assert!(features.contains_key("a"));
        assert!(features.contains_key("b"));
    }

    #[test]
    fn features_child_wins_on_conflict() {
        let mut parent_features = IndexMap::new();
        parent_features.insert("a".to_string(), serde_json::json!(1));
        let mut child_features = IndexMap::new();
        child_features.insert("a".to_string(), serde_json::json!(2));
        let parent = RawConfig {
            features: Some(parent_features),
            ..empty()
        };
        let child = RawConfig {
            features: Some(child_features),
            ..empty()
        };
        let result = merge(parent, child);
        let features = result.features.unwrap();
        assert_eq!(features["a"], serde_json::json!(2));
    }

    #[test]
    fn features_parent_order_preserved() {
        let mut parent_features = IndexMap::new();
        parent_features.insert("a".to_string(), serde_json::json!({}));
        parent_features.insert("b".to_string(), serde_json::json!({}));
        let mut child_features = IndexMap::new();
        child_features.insert("c".to_string(), serde_json::json!({}));
        let parent = RawConfig {
            features: Some(parent_features),
            ..empty()
        };
        let child = RawConfig {
            features: Some(child_features),
            ..empty()
        };
        let result = merge(parent, child);
        let features = result.features.unwrap();
        let keys: Vec<&str> = features.keys().map(|s| s.as_str()).collect();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn container_env_union() {
        let mut parent_env = HashMap::new();
        parent_env.insert("FOO".to_string(), "parent".to_string());
        parent_env.insert("BAR".to_string(), "bar".to_string());
        let mut child_env = HashMap::new();
        child_env.insert("FOO".to_string(), "child".to_string());
        child_env.insert("BAZ".to_string(), "baz".to_string());
        let parent = RawConfig {
            container_env: Some(parent_env),
            ..empty()
        };
        let child = RawConfig {
            container_env: Some(child_env),
            ..empty()
        };
        let result = merge(parent, child);
        let env = result.container_env.unwrap();
        // child wins on conflict
        assert_eq!(env["FOO"], "child");
        // parent-only key preserved
        assert_eq!(env["BAR"], "bar");
        // child-only key present
        assert_eq!(env["BAZ"], "baz");
    }

    #[test]
    fn remote_env_union() {
        let mut parent_env = HashMap::new();
        parent_env.insert("FOO".to_string(), "parent".to_string());
        parent_env.insert("BAR".to_string(), "bar".to_string());
        let mut child_env = HashMap::new();
        child_env.insert("FOO".to_string(), "child".to_string());
        child_env.insert("BAZ".to_string(), "baz".to_string());
        let parent = RawConfig {
            remote_env: Some(parent_env),
            ..empty()
        };
        let child = RawConfig {
            remote_env: Some(child_env),
            ..empty()
        };
        let result = merge(parent, child);
        let env = result.remote_env.unwrap();
        // child wins on conflict
        assert_eq!(env["FOO"], "child");
        // parent-only key preserved
        assert_eq!(env["BAR"], "bar");
        // child-only key present
        assert_eq!(env["BAZ"], "baz");
    }

    #[test]
    fn container_user_child_wins() {
        let parent = RawConfig {
            container_user: Some("root".to_string()),
            ..empty()
        };
        let child = RawConfig {
            container_user: Some("dev".to_string()),
            ..empty()
        };
        let result = merge(parent, child);
        assert_eq!(result.container_user.as_deref(), Some("dev"));
    }

    #[test]
    fn mounts_union_no_duplicates() {
        let parent = RawConfig {
            mounts: Some(vec!["A".to_string(), "B".to_string()]),
            ..empty()
        };
        let child = RawConfig {
            mounts: Some(vec!["B".to_string(), "C".to_string()]),
            ..empty()
        };
        let result = merge(parent, child);
        assert_eq!(
            result.mounts.unwrap(),
            vec!["A".to_string(), "B".to_string(), "C".to_string()]
        );
    }

    #[test]
    fn mounts_parent_order_first() {
        let parent = RawConfig {
            mounts: Some(vec!["first".to_string(), "second".to_string()]),
            ..empty()
        };
        let child = RawConfig {
            mounts: Some(vec!["third".to_string()]),
            ..empty()
        };
        let result = merge(parent, child);
        let mounts = result.mounts.unwrap();
        assert_eq!(mounts[0], "first");
        assert_eq!(mounts[1], "second");
        assert_eq!(mounts[2], "third");
    }

    #[test]
    fn final_compatibility_fields_merge_by_policy() {
        let mut parent_ports = HashMap::new();
        parent_ports.insert(
            "3000".to_string(),
            crate::config::PortAttributes {
                label: Some("parent".to_string()),
                protocol: None,
                on_auto_forward: None,
            },
        );
        let mut child_ports = HashMap::new();
        child_ports.insert(
            "3000".to_string(),
            crate::config::PortAttributes {
                label: Some("child".to_string()),
                protocol: Some("http".to_string()),
                on_auto_forward: Some(crate::config::OnAutoForward::Silent),
            },
        );
        child_ports.insert("3001".to_string(), crate::config::PortAttributes::default());

        let parent = RawConfig {
            run_args: Some(vec!["--dns=1.1.1.1".to_string()]),
            privileged: Some(false),
            cap_add: Some(vec!["NET_ADMIN".to_string()]),
            security_opt: Some(vec!["label=disable".to_string()]),
            ports_attributes: Some(parent_ports),
            other_ports_attributes: Some(crate::config::PortAttributes {
                label: Some("parent-other".to_string()),
                protocol: None,
                on_auto_forward: None,
            }),
            override_command: Some(true),
            workspace_folder: Some("/workspace/parent".to_string()),
            workspace_mount: Some(serde_json::json!("parent-mount")),
            ..empty()
        };
        let child = RawConfig {
            run_args: Some(vec![
                "--dns=1.1.1.1".to_string(),
                "--hostname=dev".to_string(),
            ]),
            privileged: Some(true),
            cap_add: Some(vec!["SYS_PTRACE".to_string()]),
            security_opt: Some(vec!["seccomp=unconfined".to_string()]),
            ports_attributes: Some(child_ports),
            other_ports_attributes: Some(crate::config::PortAttributes {
                label: Some("child-other".to_string()),
                protocol: Some("https".to_string()),
                on_auto_forward: Some(crate::config::OnAutoForward::Ignore),
            }),
            override_command: Some(false),
            workspace_folder: Some("/workspace/child".to_string()),
            workspace_mount: Some(serde_json::json!("child-mount")),
            ..empty()
        };

        let result = merge(parent, child);
        assert_eq!(
            result.run_args.unwrap(),
            vec!["--dns=1.1.1.1".to_string(), "--hostname=dev".to_string()]
        );
        assert_eq!(result.privileged, Some(true));
        assert_eq!(
            result.cap_add.unwrap(),
            vec!["NET_ADMIN".to_string(), "SYS_PTRACE".to_string()]
        );
        assert_eq!(
            result.security_opt.unwrap(),
            vec![
                "label=disable".to_string(),
                "seccomp=unconfined".to_string()
            ]
        );
        let ports = result.ports_attributes.unwrap();
        assert_eq!(ports["3000"].label.as_deref(), Some("child"));
        assert_eq!(ports["3001"], crate::config::PortAttributes::default());
        assert_eq!(
            result.other_ports_attributes.unwrap().label.as_deref(),
            Some("child-other")
        );
        assert_eq!(result.override_command, Some(false));
        assert_eq!(result.workspace_folder.as_deref(), Some("/workspace/child"));
        assert_eq!(
            result.workspace_mount,
            Some(serde_json::json!("child-mount"))
        );
    }

    #[test]
    fn forward_ports_union_dedup() {
        let parent = RawConfig {
            forward_ports: Some(vec![80, 443]),
            ..empty()
        };
        let child = RawConfig {
            forward_ports: Some(vec![443, 8080]),
            ..empty()
        };
        let result = merge(parent, child);
        assert_eq!(result.forward_ports.unwrap(), vec![80u16, 443u16, 8080u16]);
    }

    #[test]
    fn on_create_command_child_wins_not_merged() {
        let parent = RawConfig {
            on_create_command: Some(crate::lifecycle::LifecycleCommand::Shell(
                "echo parent".to_string(),
            )),
            ..empty()
        };
        let child = RawConfig {
            on_create_command: Some(crate::lifecycle::LifecycleCommand::Shell(
                "echo child".to_string(),
            )),
            ..empty()
        };
        let result = merge(parent, child);
        assert_eq!(
            result.on_create_command,
            Some(crate::lifecycle::LifecycleCommand::Shell(
                "echo child".to_string()
            ))
        );
    }

    #[test]
    fn on_create_command_child_none_uses_parent() {
        let parent = RawConfig {
            on_create_command: Some(crate::lifecycle::LifecycleCommand::Shell(
                "echo parent".to_string(),
            )),
            ..empty()
        };
        let child = empty();
        let result = merge(parent, child);
        assert_eq!(
            result.on_create_command,
            Some(crate::lifecycle::LifecycleCommand::Shell(
                "echo parent".to_string()
            ))
        );
    }

    #[test]
    fn merge_with_empty_parent() {
        let config = RawConfig {
            image: Some("rust:latest".to_string()),
            container_user: Some("dev".to_string()),
            features: Some({
                let mut m = IndexMap::new();
                m.insert("f".to_string(), serde_json::json!({}));
                m
            }),
            container_env: Some({
                let mut m = HashMap::new();
                m.insert("K".to_string(), "V".to_string());
                m
            }),
            mounts: Some(vec!["m".to_string()]),
            forward_ports: Some(vec![8080]),
            ..empty()
        };
        let result = merge(empty(), config);
        assert_eq!(result.image.as_deref(), Some("rust:latest"));
        assert_eq!(result.container_user.as_deref(), Some("dev"));
        let features = result.features.unwrap();
        assert!(features.contains_key("f"));
        let env = result.container_env.unwrap();
        assert_eq!(env["K"], "V");
        assert_eq!(result.mounts.as_deref(), Some(&["m".to_string()][..]));
        assert_eq!(result.forward_ports.as_deref(), Some(&[8080u16][..]));
    }

    #[test]
    fn merge_with_empty_child() {
        let config = RawConfig {
            image: Some("rust:latest".to_string()),
            container_user: Some("dev".to_string()),
            features: Some({
                let mut m = IndexMap::new();
                m.insert("f".to_string(), serde_json::json!({}));
                m
            }),
            container_env: Some({
                let mut m = HashMap::new();
                m.insert("K".to_string(), "V".to_string());
                m
            }),
            mounts: Some(vec!["m".to_string()]),
            forward_ports: Some(vec![8080]),
            ..empty()
        };
        let result = merge(config, empty());
        assert_eq!(result.image.as_deref(), Some("rust:latest"));
        assert_eq!(result.container_user.as_deref(), Some("dev"));
        let features = result.features.unwrap();
        assert!(features.contains_key("f"));
        let env = result.container_env.unwrap();
        assert_eq!(env["K"], "V");
        assert_eq!(result.mounts.as_deref(), Some(&["m".to_string()][..]));
        assert_eq!(result.forward_ports.as_deref(), Some(&[8080u16][..]));
    }

    #[test]
    fn scripts_child_wins_on_conflict() {
        let mut parent_scripts = HashMap::new();
        parent_scripts.insert("build".to_string(), "make parent".to_string());
        parent_scripts.insert("test".to_string(), "make test".to_string());
        let mut child_scripts = HashMap::new();
        child_scripts.insert("build".to_string(), "cargo build".to_string());
        child_scripts.insert("lint".to_string(), "cargo clippy".to_string());
        let parent = RawConfig {
            scripts: Some(parent_scripts),
            ..empty()
        };
        let child = RawConfig {
            scripts: Some(child_scripts),
            ..empty()
        };
        let result = merge(parent, child);
        let scripts = result.scripts.unwrap();
        assert_eq!(scripts["build"], "cargo build");
        assert_eq!(scripts["test"], "make test");
        assert_eq!(scripts["lint"], "cargo clippy");
    }

    #[test]
    fn dcc_commands_child_wins_on_conflict() {
        let mut parent_commands = HashMap::new();
        parent_commands.insert("build".to_string(), "make parent".to_string());
        parent_commands.insert("test".to_string(), "make test".to_string());
        let mut child_commands = HashMap::new();
        child_commands.insert("build".to_string(), "cargo build".to_string());
        child_commands.insert("lint".to_string(), "cargo clippy".to_string());
        let parent = RawConfig {
            customizations: Some(Customizations {
                dcc: Some(RawDccConfig {
                    commands: Some(parent_commands),
                    ..RawDccConfig::default()
                }),
                ..Customizations::default()
            }),
            ..empty()
        };
        let child = RawConfig {
            customizations: Some(Customizations {
                dcc: Some(RawDccConfig {
                    commands: Some(child_commands),
                    ..RawDccConfig::default()
                }),
                ..Customizations::default()
            }),
            ..empty()
        };
        let result = merge(parent, child);
        let commands = result
            .customizations
            .and_then(|c| c.dcc)
            .and_then(|dcc| dcc.commands)
            .unwrap();
        assert_eq!(commands["build"], "cargo build");
        assert_eq!(commands["test"], "make test");
        assert_eq!(commands["lint"], "cargo clippy");
    }

    #[test]
    fn dcc_registry_cas_union_by_canonical_authority_with_child_wins() {
        use crate::config::registry_ca::{RawRegistryCas, RegistryAuthority};

        let parent_cas: RawRegistryCas = json5::from_str(
            r#"{"registry.test:443":"parent.pem","parent.test":"parent-only.pem"}"#,
        )
        .unwrap();
        let child_cas: RawRegistryCas =
            json5::from_str(r#"{"REGISTRY.test":"child.pem","child.test:5443":"child-only.pem"}"#)
                .unwrap();
        let parent = RawConfig {
            customizations: Some(Customizations {
                dcc: Some(RawDccConfig {
                    registry_cas: Some(parent_cas),
                    ..RawDccConfig::default()
                }),
                ..Customizations::default()
            }),
            ..empty()
        };
        let child = RawConfig {
            customizations: Some(Customizations {
                dcc: Some(RawDccConfig {
                    registry_cas: Some(child_cas),
                    ..RawDccConfig::default()
                }),
                ..Customizations::default()
            }),
            ..empty()
        };

        let registry_cas = merge(parent, child)
            .customizations
            .unwrap()
            .dcc
            .unwrap()
            .registry_cas
            .unwrap();
        assert_eq!(registry_cas.0.len(), 3);
        assert_eq!(
            registry_cas.path_for(&RegistryAuthority::parse("registry.test").unwrap()),
            Some(std::path::Path::new("child.pem"))
        );
        assert!(registry_cas
            .path_for(&RegistryAuthority::parse("parent.test").unwrap())
            .is_some());
        assert!(registry_cas
            .path_for(&RegistryAuthority::parse("child.test:5443").unwrap())
            .is_some());
    }

    #[test]
    fn dcc_extends_always_none() {
        let parent = RawConfig {
            customizations: Some(Customizations {
                dcc: Some(RawDccConfig {
                    extends: Some("parent-base.json".to_string()),
                    ..RawDccConfig::default()
                }),
                ..Customizations::default()
            }),
            ..empty()
        };
        let child = RawConfig {
            customizations: Some(Customizations {
                dcc: Some(RawDccConfig {
                    extends: Some("child-base.json".to_string()),
                    ..RawDccConfig::default()
                }),
                ..Customizations::default()
            }),
            ..empty()
        };
        let result = merge(parent, child);
        assert!(result
            .customizations
            .and_then(|c| c.dcc)
            .and_then(|dcc| dcc.extends)
            .is_none());
    }

    #[test]
    fn dcc_state_union_no_duplicates() {
        let parent = RawConfig {
            customizations: Some(Customizations {
                dcc: Some(RawDccConfig {
                    state: Some(vec![
                        crate::config::StateEntry {
                            path: "/cache/a".to_string(),
                            kind: crate::config::StateKind::Directory,
                        },
                        crate::config::StateEntry {
                            path: "/cache/b".to_string(),
                            kind: crate::config::StateKind::File,
                        },
                    ]),
                    ..RawDccConfig::default()
                }),
                ..Customizations::default()
            }),
            ..empty()
        };
        let child = RawConfig {
            customizations: Some(Customizations {
                dcc: Some(RawDccConfig {
                    state: Some(vec![
                        crate::config::StateEntry {
                            path: "/cache/a".to_string(),
                            kind: crate::config::StateKind::Directory,
                        },
                        crate::config::StateEntry {
                            path: "/cache/c".to_string(),
                            kind: crate::config::StateKind::Directory,
                        },
                    ]),
                    ..RawDccConfig::default()
                }),
                ..Customizations::default()
            }),
            ..empty()
        };
        let result = merge(parent, child);
        let state = result
            .customizations
            .and_then(|c| c.dcc)
            .and_then(|dcc| dcc.state)
            .unwrap();
        assert_eq!(
            state,
            vec![
                crate::config::StateEntry {
                    path: "/cache/a".to_string(),
                    kind: crate::config::StateKind::Directory,
                },
                crate::config::StateEntry {
                    path: "/cache/b".to_string(),
                    kind: crate::config::StateKind::File,
                },
                crate::config::StateEntry {
                    path: "/cache/c".to_string(),
                    kind: crate::config::StateKind::Directory,
                },
            ]
        );
    }

    fn arb_text() -> BoxedStrategy<String> {
        "[a-z0-9._/-]{0,12}".boxed()
    }

    fn arb_string_map() -> BoxedStrategy<Option<HashMap<String, String>>> {
        proptest::option::of(proptest::collection::hash_map(
            "[a-e]{1,3}",
            "[a-z0-9._/-]{0,8}",
            0..5,
        ))
        .boxed()
    }

    fn arb_string_list() -> BoxedStrategy<Option<Vec<String>>> {
        proptest::option::of(proptest::collection::vec("[a-f]{1,3}", 0..6)).boxed()
    }

    fn arb_features() -> BoxedStrategy<Option<IndexMap<String, serde_json::Value>>> {
        proptest::option::of(
            proptest::collection::vec(("feature-[a-d]", any::<u8>()), 0..5).prop_map(|pairs| {
                pairs
                    .into_iter()
                    .map(|(key, value)| (key, serde_json::json!({ "value": value })))
                    .collect()
            }),
        )
        .boxed()
    }

    fn arb_lifecycle() -> BoxedStrategy<Option<crate::lifecycle::LifecycleCommand>> {
        let single = prop_oneof![
            arb_text().prop_map(crate::lifecycle::LifecycleCommandSingle::Shell),
            proptest::collection::vec(arb_text(), 0..4)
                .prop_map(crate::lifecycle::LifecycleCommandSingle::Exec),
        ];
        proptest::option::of(prop_oneof![
            arb_text().prop_map(crate::lifecycle::LifecycleCommand::Shell),
            proptest::collection::vec(arb_text(), 0..5)
                .prop_map(crate::lifecycle::LifecycleCommand::Exec),
            proptest::collection::vec(("hook-[a-d]", single), 0..4).prop_map(|entries| {
                crate::lifecycle::LifecycleCommand::Parallel(entries.into_iter().collect())
            }),
        ])
        .boxed()
    }

    fn arb_build() -> BoxedStrategy<Option<crate::config::BuildConfig>> {
        proptest::option::of(
            (arb_text(), arb_text(), proptest::option::of(arb_text())).prop_map(
                |(context, dockerfile, target)| crate::config::BuildConfig {
                    context,
                    dockerfile,
                    args: HashMap::new(),
                    target,
                },
            ),
        )
        .boxed()
    }

    fn arb_customizations() -> BoxedStrategy<Option<Customizations>> {
        let state = proptest::option::of(proptest::collection::vec(
            ("/[a-e]{1,4}", any::<bool>()).prop_map(|(path, is_file)| crate::config::StateEntry {
                path,
                kind: if is_file {
                    crate::config::StateKind::File
                } else {
                    crate::config::StateKind::Directory
                },
            }),
            0..5,
        ));
        proptest::option::of((arb_string_map(), state, arb_string_map()).prop_map(
            |(commands, state, other)| {
                Customizations {
                    dcc: Some(RawDccConfig {
                        extends: None,
                        commands,
                        state,
                        registry_cas: None,
                        extra: HashMap::new(),
                    }),
                    other: other
                        .unwrap_or_default()
                        .into_iter()
                        .map(|(key, value)| (key, serde_json::json!(value)))
                        .collect(),
                }
            },
        ))
        .boxed()
    }

    fn arb_raw_config() -> BoxedStrategy<RawConfig> {
        let scalars = (
            proptest::option::of(arb_text()),
            proptest::option::of(arb_text()),
            proptest::option::of(arb_text()),
            proptest::option::of(any::<bool>()),
            proptest::option::of(any::<bool>()),
            proptest::option::of(any::<bool>()),
            proptest::option::of(arb_text()),
            proptest::option::of(any::<u8>().prop_map(|value| serde_json::json!(value))),
        );
        let maps = (arb_string_map(), arb_string_map(), arb_string_map());
        let lists = (
            arb_string_list(),
            arb_string_list(),
            arb_string_list(),
            arb_string_list(),
            proptest::option::of(proptest::collection::vec(1u16..10000, 0..6)),
        );
        let hooks = (arb_lifecycle(), arb_lifecycle(), arb_lifecycle());

        (
            scalars,
            maps,
            lists,
            arb_features(),
            hooks,
            arb_customizations(),
            arb_build(),
        )
            .prop_map(
                |(
                    (
                        name,
                        image,
                        container_user,
                        privileged,
                        override_command,
                        update_uid,
                        workspace_folder,
                        workspace_mount,
                    ),
                    (container_env, remote_env, scripts),
                    (mounts, run_args, cap_add, security_opt, forward_ports),
                    features,
                    (initialize_command, on_create_command, post_start_command),
                    customizations,
                    build,
                )| RawConfig {
                    extends: None,
                    name,
                    image,
                    build,
                    features,
                    container_env,
                    remote_env,
                    container_user,
                    mounts,
                    run_args,
                    privileged,
                    cap_add,
                    security_opt,
                    forward_ports,
                    ports_attributes: None,
                    other_ports_attributes: None,
                    override_command,
                    update_remote_user_uid: update_uid,
                    workspace_folder,
                    workspace_mount,
                    initialize_command,
                    on_create_command,
                    update_content_command: None,
                    post_create_command: None,
                    post_start_command,
                    post_attach_command: None,
                    scripts,
                    customizations,
                    extra: HashMap::new(),
                },
            )
            .boxed()
    }

    fn expected_option_map<V: Clone>(
        parent: Option<HashMap<String, V>>,
        child: Option<HashMap<String, V>>,
    ) -> Option<HashMap<String, V>> {
        match (parent, child) {
            (None, None) => None,
            (value, None) | (None, value) => value,
            (Some(mut parent), Some(child)) => {
                parent.extend(child);
                Some(parent)
            }
        }
    }

    fn expected_option_vec<T: Clone + Eq + std::hash::Hash>(
        parent: Option<Vec<T>>,
        child: Option<Vec<T>>,
    ) -> Option<Vec<T>> {
        match (parent, child) {
            (None, None) => None,
            (value, None) | (None, value) => value,
            (Some(parent), Some(child)) => {
                let mut seen = HashSet::new();
                Some(
                    parent
                        .into_iter()
                        .chain(child)
                        .filter(|value| seen.insert(value.clone()))
                        .collect(),
                )
            }
        }
    }

    fn expected_features(
        parent: Option<IndexMap<String, serde_json::Value>>,
        child: Option<IndexMap<String, serde_json::Value>>,
    ) -> Option<IndexMap<String, serde_json::Value>> {
        match (parent, child) {
            (None, None) => None,
            (value, None) | (None, value) => value,
            (Some(mut parent), Some(child)) => {
                parent.extend(child);
                Some(parent)
            }
        }
    }

    fn expected_customizations(
        parent: Option<Customizations>,
        child: Option<Customizations>,
    ) -> Option<Customizations> {
        match (parent, child) {
            (None, None) => None,
            (value, None) | (None, value) => value,
            (Some(parent), Some(child)) => {
                let dcc = match (parent.dcc, child.dcc) {
                    (None, None) => None,
                    (value, None) | (None, value) => value,
                    (Some(parent), Some(child)) => Some(RawDccConfig {
                        extends: None,
                        commands: expected_option_map(parent.commands, child.commands),
                        state: expected_option_vec(parent.state, child.state),
                        registry_cas: match (parent.registry_cas, child.registry_cas) {
                            (None, None) => None,
                            (value, None) | (None, value) => value,
                            (Some(parent), Some(child)) => Some(parent.merge(child)),
                        },
                        extra: {
                            let mut extra = parent.extra;
                            extra.extend(child.extra);
                            extra
                        },
                    }),
                };
                let mut other = parent.other;
                other.extend(child.other);
                Some(Customizations { dcc, other })
            }
        }
    }

    fn expected_merge(parent: RawConfig, child: RawConfig) -> RawConfig {
        RawConfig {
            extends: None,
            name: child.name.or(parent.name),
            image: child.image.or(parent.image),
            build: child.build.or(parent.build),
            features: expected_features(parent.features, child.features),
            container_env: expected_option_map(parent.container_env, child.container_env),
            remote_env: expected_option_map(parent.remote_env, child.remote_env),
            container_user: child.container_user.or(parent.container_user),
            mounts: expected_option_vec(parent.mounts, child.mounts),
            run_args: expected_option_vec(parent.run_args, child.run_args),
            privileged: child.privileged.or(parent.privileged),
            cap_add: expected_option_vec(parent.cap_add, child.cap_add),
            security_opt: expected_option_vec(parent.security_opt, child.security_opt),
            forward_ports: expected_option_vec(parent.forward_ports, child.forward_ports),
            ports_attributes: None,
            other_ports_attributes: None,
            override_command: child.override_command.or(parent.override_command),
            update_remote_user_uid: child
                .update_remote_user_uid
                .or(parent.update_remote_user_uid),
            workspace_folder: child.workspace_folder.or(parent.workspace_folder),
            workspace_mount: child.workspace_mount.or(parent.workspace_mount),
            initialize_command: child.initialize_command.or(parent.initialize_command),
            on_create_command: child.on_create_command.or(parent.on_create_command),
            update_content_command: None,
            post_create_command: None,
            post_start_command: child.post_start_command.or(parent.post_start_command),
            post_attach_command: None,
            scripts: expected_option_map(parent.scripts, child.scripts),
            customizations: expected_customizations(parent.customizations, child.customizations),
            extra: HashMap::new(),
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig {
            cases: 96,
            failure_persistence: None,
            rng_seed: proptest::test_runner::RngSeed::Fixed(0xDCC0_0053),
            ..ProptestConfig::default()
        })]

        #[test]
        fn merge_obeys_precedence_union_and_order_laws(
            parent in arb_raw_config(),
            child in arb_raw_config(),
        ) {
            let expected = expected_merge(parent.clone(), child.clone());
            prop_assert_eq!(merge(parent, child), expected);
        }

        #[test]
        fn empty_config_is_a_two_sided_identity(config in arb_raw_config()) {
            prop_assert_eq!(merge(empty(), config.clone()), config.clone());
            prop_assert_eq!(merge(config.clone(), empty()), config);
        }
    }
}
