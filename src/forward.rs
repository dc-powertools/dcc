use anyhow::Context as _;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PortMapping {
    pub(crate) target: u16,
    pub(crate) proxy: u16,
}

pub(crate) fn plan_ports(ports: &[u16], range: [u16; 2]) -> anyhow::Result<Vec<PortMapping>> {
    anyhow::ensure!(
        range[0] >= 1024 && range[0] <= range[1],
        "customizations.dcc.relayPortRange must be an inclusive [start, end] in 1024..65535"
    );
    let targets: std::collections::BTreeSet<_> = ports.iter().copied().collect();
    anyhow::ensure!(!targets.contains(&0), "forwardPorts cannot contain port 0");
    let mut available = (range[0]..=range[1]).filter(|p| !targets.contains(p));
    targets
        .iter()
        .map(|target| {
            Ok(PortMapping {
                target: *target,
                proxy: available
                    .next()
                    .context("relayPortRange exhausted; enlarge the range")?,
            })
        })
        .collect()
}

pub(crate) fn publication_args(ports: &[PortMapping], ipv6: bool) -> Vec<String> {
    ports
        .iter()
        .flat_map(|p| {
            let mut args = vec![
                "--publish".into(),
                format!("127.0.0.1:{}:{}/tcp", p.target, p.proxy),
            ];
            if ipv6 {
                args.extend([
                    "--publish".into(),
                    format!("[::1]:{}:{}/tcp", p.target, p.proxy),
                ]);
            }
            args
        })
        .collect()
}

pub(crate) fn baked_relay_assets() -> Vec<(String, Vec<u8>, u32)> {
    [
        ("dcc-relay", include_str!("relay.sh")),
        ("dcc-relay-service", include_str!("relay_service.sh")),
    ]
    .into_iter()
    .map(|(name, script)| {
        (
            format!(".dcc-generated/{name}"),
            script.as_bytes().to_vec(),
            0o755,
        )
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn allocation_is_order_independent_deduplicated_and_skips_targets() {
        let expected = vec![
            PortMapping {
                target: 4173,
                proxy: 20001,
            },
            PortMapping {
                target: 20000,
                proxy: 20002,
            },
        ];
        assert_eq!(
            plan_ports(&[20000, 4173, 4173], [20000, 20002]).unwrap(),
            expected
        );
        assert_eq!(
            plan_ports(&[4173, 20000], [20000, 20002]).unwrap(),
            expected
        );
        assert!(plan_ports(&[4173, 20000], [20000, 20001]).is_err());
    }
    #[test]
    fn range_and_port_boundaries() {
        for range in [[0, 20000], [1023, 20000], [20001, 20000]] {
            assert!(plan_ports(&[], range).is_err());
        }
        assert!(plan_ports(&[0], [20000, 20999]).is_err());
        assert_eq!(plan_ports(&[80], [65535, 65535]).unwrap()[0].proxy, 65535);
        assert!(plan_ports(&[], [20000, 20999]).unwrap().is_empty());
    }
    #[test]
    fn publication_is_explicit_loopback_only() {
        let plan = plan_ports(&[4173], [20000, 20999]).unwrap();
        assert_eq!(
            publication_args(&plan, true),
            [
                "--publish",
                "127.0.0.1:4173:20000/tcp",
                "--publish",
                "[::1]:4173:20000/tcp"
            ]
        );
        assert_eq!(
            publication_args(&plan, false),
            ["--publish", "127.0.0.1:4173:20000/tcp"]
        );
    }
    #[cfg(unix)]
    #[test]
    fn relay_wrapper_executes_fixed_socat_argv_and_rejects_injection() {
        use std::{fs, os::unix::fs::PermissionsExt as _, process::Command};
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("relay");
        fs::write(&script, include_str!("relay.sh")).unwrap();
        let fake = temp.path().join("socat");
        fs::write(&fake, "#!/bin/sh\nprintf '%s\\n' \"$@\"\n").unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        let output = Command::new("/bin/sh")
            .arg(&script)
            .args(["20000", "4173"])
            .env("PATH", temp.path())
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "-t\n2\nTCP4-LISTEN:20000,bind=0.0.0.0,reuseaddr,fork\nTCP4:127.0.0.1:4173\n"
        );
        for port in ["4173,exec=bad", "0", "65536", "-1", "$(bad)"] {
            let output = Command::new("/bin/sh")
                .arg(&script)
                .args(["20000", port])
                .env("PATH", temp.path())
                .output()
                .unwrap();
            assert!(!output.status.success());
            assert!(output.stdout.is_empty());
        }
        for (_, content, _) in baked_relay_assets() {
            let mut child = Command::new("sh")
                .arg("-n")
                .stdin(std::process::Stdio::piped())
                .spawn()
                .unwrap();
            use std::io::Write as _;
            child.stdin.take().unwrap().write_all(&content).unwrap();
            assert!(child.wait().unwrap().success());
        }
    }
}
