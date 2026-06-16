use std::collections::{HashMap, HashSet};

use indexmap::IndexMap;

use crate::config::RawConfig;

pub(crate) fn merge(parent: RawConfig, child: RawConfig) -> RawConfig {
    RawConfig {
        extends: None,
        image: child.image.or(parent.image),
        features: merge_option_index_maps(parent.features, child.features),
        container_env: merge_option_hash_maps(parent.container_env, child.container_env),
        remote_env: merge_option_hash_maps(parent.remote_env, child.remote_env),
        container_user: child.container_user.or(parent.container_user),
        mounts: merge_option_vecs(parent.mounts, child.mounts),
        forward_ports: merge_option_vecs(parent.forward_ports, child.forward_ports),
        initialize_command: child.initialize_command.or(parent.initialize_command),
        on_create_command: child.on_create_command.or(parent.on_create_command),
        update_content_command: child
            .update_content_command
            .or(parent.update_content_command),
        post_create_command: child.post_create_command.or(parent.post_create_command),
        post_start_command: child.post_start_command.or(parent.post_start_command),
        post_attach_command: child.post_attach_command.or(parent.post_attach_command),
        extra: merge_hash_maps(parent.extra, child.extra),
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
            image: None,
            features: None,
            container_env: None,
            remote_env: None,
            container_user: None,
            mounts: None,
            forward_ports: None,
            initialize_command: None,
            on_create_command: None,
            update_content_command: None,
            post_create_command: None,
            post_start_command: None,
            post_attach_command: None,
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

    proptest! {
        #[test]
        fn merge_with_empty_is_stable(image in proptest::option::of("[a-z0-9:.-]+")) {
            let config = RawConfig { image: image.clone(), ..empty() };
            let once = merge(empty(), config);
            // merging child with empty parent should return child unchanged
            assert_eq!(once.image, image);
        }
    }
}
