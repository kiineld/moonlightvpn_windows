//! Tests that need the real mihomo binary, or a real Windows machine.
//!
//! The unit tests assert that this client *builds* the config it means to. That
//! is not the same as the core accepting it: mihomo refuses a whole config for
//! one malformed rule, and the grammar it accepts in a `SUB-RULE` matcher is not
//! the grammar it accepts in a plain rule. A config this client is sure about
//! and the core rejects is a tunnel that stops with no UI explanation, so every
//! shape goes through `mihomo -t` here.
//!
//! Everything is skipped when its prerequisite is absent, and says so, rather
//! than failing — a developer without the core checked out should still be able
//! to run `cargo test`.
//!
//! - `MOONLIGHT_MIHOMO` points at `mihomo.exe`. CI sets it after fetching.
//! - `MOONLIGHT_ADMIN_TESTS=1` opts in to the ones that install a service.
//!   They are off by default because they change machine state.

use std::path::PathBuf;
use std::process::Command;

use moonlight_core::mihomo_config::{self, Overrides};
use moonlight_core::split_rule::{Kind, SplitRule};
use moonlight_core::{SplitMode, TunnelMode};

const PANEL: &str = r#"
proxies:
  - name: "Node A"
    type: ss
    server: 127.0.0.1
    port: 8388
    cipher: aes-256-gcm
    password: secret
    udp: true
  - name: "Node B"
    type: ss
    server: 127.0.0.1
    port: 8389
    cipher: aes-256-gcm
    password: secret
    udp: true
proxy-groups:
  - name: "PANEL-SELECT"
    type: select
    proxies: ["Node A", "Node B"]
rules:
  - "DOMAIN-SUFFIX,example.com,DIRECT"
  - "MATCH,PANEL-SELECT"
"#;

fn core() -> Option<PathBuf> {
    let path = PathBuf::from(std::env::var_os("MOONLIGHT_MIHOMO")?);
    path.is_file().then_some(path)
}

fn overrides() -> Overrides {
    Overrides {
        secret: "test-secret".into(),
        // Ports well away from anything a developer is likely to be running.
        controller_port: 29797,
        mixed_port: 27897,
        ..Default::default()
    }
}

/// Runs `mihomo -t` over a config and returns its complaint, if it had one.
///
/// The core is given its own directory per call: it writes a cache and, on a
/// cold start, downloads geo databases, and two tests sharing one directory race
/// over both.
fn validate(config: &str, label: &str) -> Result<(), String> {
    let Some(binary) = core() else {
        return Ok(());
    };
    let directory = std::env::temp_dir().join(format!("moonlight-test-{label}"));
    let _ = std::fs::create_dir_all(&directory);
    let path = directory.join("config.yaml");
    std::fs::write(&path, config).expect("the temporary directory is writable");

    let output = Command::new(&binary)
        .arg("-t")
        .arg("-d")
        .arg(&directory)
        .arg("-f")
        .arg(&path)
        .output()
        .map_err(|e| format!("could not run the core: {e}"))?;

    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "{label}: the core refused the config\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

#[test]
fn the_core_accepts_a_plain_system_proxy_config() {
    if core().is_none() {
        eprintln!("skipped: MOONLIGHT_MIHOMO is not set");
        return;
    }
    let config = mihomo_config::build(PANEL, &overrides()).expect("builds");
    validate(&config, "system-proxy").expect("the core must accept it");
}

#[test]
fn the_core_accepts_a_tun_config() {
    // Validated, never started: a test suite must not ask for Administrator,
    // and starting this one would create a Wintun adapter and install routes.
    if core().is_none() {
        eprintln!("skipped: MOONLIGHT_MIHOMO is not set");
        return;
    }
    let mut o = overrides();
    o.mode = TunnelMode::Tun;
    let config = mihomo_config::build(PANEL, &o).expect("builds");
    assert!(config.contains("tun:"), "the TUN block must be present");
    validate(&config, "tun").expect("the core must accept it");
}

/// Every rule kind, in **both** positions.
///
/// This is the test the macOS client's suite exists for: mihomo accepts
/// different grammars as a plain rule and inside a `SUB-RULE` matcher, so a rule
/// that only works in one produces a config the core refuses — and it refuses
/// the whole thing, so one bad kind stops the tunnel rather than being skipped.
#[test]
fn every_rule_kind_is_accepted_in_both_positions() {
    if core().is_none() {
        eprintln!("skipped: MOONLIGHT_MIHOMO is not set");
        return;
    }

    for kind in Kind::ALL {
        let rule = SplitRule::new(*kind, kind.placeholder());

        // Position one: a plain rule, which is what "except these" writes.
        let mut except = overrides();
        except.mode = TunnelMode::Tun; // so PROCESS-* rules are not filtered out
        except.split_mode = SplitMode::Except;
        except.split_rules = vec![rule.clone()];
        let config = mihomo_config::build(PANEL, &except).expect("builds");
        validate(&config, &format!("except-{}", kind.token().to_lowercase()))
            .unwrap_or_else(|why| panic!("{} as a plain rule: {why}", kind.token()));

        // Position two: inside a SUB-RULE matcher, which is what "only these"
        // writes.
        let mut only = overrides();
        only.mode = TunnelMode::Tun;
        only.split_mode = SplitMode::Only;
        only.split_rules = vec![rule];
        let config = mihomo_config::build(PANEL, &only).expect("builds");
        validate(&config, &format!("only-{}", kind.token().to_lowercase()))
            .unwrap_or_else(|why| panic!("{} inside a SUB-RULE: {why}", kind.token()));
    }
}

#[test]
fn the_core_accepts_the_share_link_fallback_shape() {
    if core().is_none() {
        eprintln!("skipped: MOONLIGHT_MIHOMO is not set");
        return;
    }
    // The shape produced when a panel serves no Clash config at all.
    let links = [
        "ss://YWVzLTI1Ni1nY206c2VjcmV0@127.0.0.1:8388#Node%20A",
        "trojan://password@127.0.0.1:8443?sni=example.com#Node%20B",
    ];
    let proxies: Vec<_> = links
        .iter()
        .filter_map(|l| moonlight_core::share_link::mihomo_proxy(l))
        .collect();
    assert_eq!(proxies.len(), 2, "both links must parse");

    let yaml = mihomo_config::yaml_from_proxies(&proxies);
    let config = mihomo_config::build(&yaml, &overrides()).expect("builds");
    validate(&config, "share-links").expect("the core must accept it");
}

/// Starts a core for real and drives its RESTful API.
///
/// The API is this client's entire control channel, so it is exercised rather
/// than assumed. System-proxy mode only: no privileges, no interface, no routes.
#[tokio::test]
async fn a_started_core_answers_the_api_this_client_speaks() {
    let Some(binary) = core() else {
        eprintln!("skipped: MOONLIGHT_MIHOMO is not set");
        return;
    };

    let o = overrides();
    let config = mihomo_config::build(PANEL, &o).expect("builds");
    let directory = std::env::temp_dir().join("moonlight-test-live");
    let _ = std::fs::create_dir_all(&directory);
    let path = directory.join("config.yaml");
    std::fs::write(&path, &config).expect("writable");

    let mut core = moonlight_core::process::MihomoProcess::new(binary, &directory);
    core.start(&path, None).await.expect("the core starts");

    let api = moonlight_core::api::MihomoApi::new(o.controller_port, o.secret.clone());
    let ready = api
        .wait_until_ready(std::time::Duration::from_secs(60))
        .await;
    assert!(ready, "the core never answered:\n{}", core.log());

    // /version — the readiness probe itself.
    let version = api.version().await.expect("version");
    assert!(!version.is_empty());

    // /proxies — the selector and its members, which is how the node list is
    // built and how "pick a server" works.
    let groups = api.groups().await.expect("groups");
    assert!(
        groups.iter().any(|g| g.name == "PANEL-SELECT"),
        "the panel's own group must survive into the running core"
    );
    let nodes = api.nodes("PANEL-SELECT").await.expect("nodes");
    assert_eq!(nodes.len(), 2);

    // Switching a node, which the whole client is built around not needing a
    // restart for.
    api.select("Node B", "PANEL-SELECT")
        .await
        .expect("selects a node");
    let groups = api.groups().await.expect("groups");
    let selector = groups
        .iter()
        .find(|g| g.name == "PANEL-SELECT")
        .expect("the selector");
    assert_eq!(selector.now.as_deref(), Some("Node B"));

    // /connections — the shape the Connections screen reads.
    let connections = api.connections().await.expect("connections");
    assert!(connections.is_empty(), "nothing is routed through it yet");
    let _ = api.totals().await.expect("totals");

    // A live reload, which is how a refreshed subscription lands without
    // dropping the tunnel.
    api.reload(&path.to_string_lossy())
        .await
        .expect("reloads in place");

    core.stop().await;
}

// Windows-only, and only when explicitly opted into.

/// The registry proxy layer, against the real registry.
///
/// It writes `HKCU`, so it restores what it found — and it asserts the restore,
/// because a test that leaves a machine proxied at a dead port is worse than no
/// test at all.
#[cfg(windows)]
#[test]
fn the_system_proxy_writes_and_restores_the_real_registry() {
    use moonlight_core::system_proxy;

    let original = system_proxy::snapshot();

    assert!(
        system_proxy::enable(27897),
        "the proxy settings could not be written"
    );
    let written = system_proxy::snapshot();
    assert!(written.enabled, "ProxyEnable was not set");
    assert!(
        written.server.contains("27897"),
        "ProxyServer does not point at the core: {}",
        written.server
    );
    assert!(
        written.server.contains("http=") && written.server.contains("https="),
        "both protocols must be named explicitly: {}",
        written.server
    );
    assert!(
        written.bypass.contains("127.*"),
        "loopback must be bypassed, or the app's own API calls loop through the tunnel"
    );
    assert!(
        written.auto_config_url.is_none(),
        "a PAC script would take precedence over everything just written"
    );

    assert!(
        system_proxy::restore(&original),
        "the proxy settings could not be restored"
    );
    let restored = system_proxy::snapshot();
    assert_eq!(
        restored, original,
        "the machine was left with settings it did not start with"
    );
}

/// Installs the service, talks to it over the pipe, and removes it.
///
/// Off unless `MOONLIGHT_ADMIN_TESTS=1`, because it needs Administrator and
/// changes machine state. CI runs it; a developer's machine does not, unless
/// asked.
#[cfg(windows)]
#[test]
fn the_helper_installs_answers_its_pipe_and_uninstalls() {
    use moonlight_core::helper::{self, Request, Response};
    use std::time::Duration;

    if std::env::var("MOONLIGHT_ADMIN_TESTS").as_deref() != Ok("1") {
        eprintln!("skipped: set MOONLIGHT_ADMIN_TESTS=1 to run the service tests");
        return;
    }

    let helper_exe = std::env::var_os("MOONLIGHT_HELPER")
        .map(PathBuf::from)
        .expect("MOONLIGHT_HELPER must point at moonlight-helper.exe");

    let install = Command::new(&helper_exe)
        .arg("--install")
        .status()
        .expect("runs the installer");
    assert!(install.success(), "the service did not install");

    // The control manager reports a service started before it has opened its
    // pipe, so the client retries — this is that retry doing its job.
    let response = helper::send(&Request::Ping, Duration::from_secs(15))
        .expect("the helper must answer its pipe");
    assert!(
        matches!(response, Response::Pong { .. }),
        "unexpected reply: {response:?}"
    );

    assert!(
        helper::is_installed(),
        "the service should now be installed"
    );

    // A request the service must refuse. The client is not the security
    // boundary, so this is checked where the privilege is.
    let refused = helper::send(
        &Request::Start {
            config: "not a config".into(),
        },
        Duration::from_secs(10),
    )
    .expect("answers");
    assert!(
        matches!(refused, Response::Error { .. }),
        "the service accepted a config that is not one: {refused:?}"
    );

    let status = helper::send(&Request::Status, Duration::from_secs(10)).expect("answers");
    assert!(matches!(status, Response::Status { running: false }));

    let uninstall = Command::new(&helper_exe)
        .arg("--uninstall")
        .status()
        .expect("runs the uninstaller");
    assert!(uninstall.success(), "the service did not uninstall");
}

/// The app inventory, against the real machine.
#[cfg(windows)]
#[test]
fn the_inventory_finds_real_programs_on_this_machine() {
    use moonlight_core::app_inventory;

    let running = app_inventory::running_executables();
    assert!(
        !running.is_empty(),
        "the process table cannot be empty on a running Windows"
    );

    let apps = app_inventory::scan();
    assert!(
        !apps.is_empty(),
        "a Windows install always has something in its Start Menu"
    );
    for entry in &apps {
        assert!(
            !app_inventory::is_system_executable(&entry.executable),
            "{} is a Windows component, not an application",
            entry.executable
        );
        // A PROCESS-NAME rule matches the file name with its extension.
        assert!(
            entry.executable.to_lowercase().ends_with(".exe"),
            "{} has no extension, so a rule for it would never match",
            entry.executable
        );
    }
}
