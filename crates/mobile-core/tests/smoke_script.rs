use std::path::Path;
use std::process::Command;

#[test]
fn e2e_smoke_script_documents_full_cli_stack() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let script_path = workspace.join("scripts/e2e-smoke.sh");

    let metadata = std::fs::metadata(&script_path).expect("scripts/e2e-smoke.sh should exist");
    assert!(metadata.is_file());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_ne!(
            metadata.permissions().mode() & 0o111,
            0,
            "scripts/e2e-smoke.sh should be executable"
        );
    }

    let script = std::fs::read_to_string(script_path).unwrap();
    for expected in [
        "control-server",
        "punch-server",
        "relayd",
        "agentd",
        "mobile-cli",
        "--p2p-identity-dir",
        "--relay-fallback-delay-ms",
        "--state-db",
        "assert_admin_session_visible",
        "assert_http_forward_response",
        "quic-test-forward-ok",
        "run_case p2p",
        "run_case fallback",
    ] {
        assert!(script.contains(expected), "missing {expected}");
    }
}

#[test]
fn dev_stack_script_documents_manual_start_order() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let script_path = workspace.join("scripts/dev-stack.sh");

    let metadata = std::fs::metadata(&script_path).expect("scripts/dev-stack.sh should exist");
    assert!(metadata.is_file());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_ne!(
            metadata.permissions().mode() & 0o111,
            0,
            "scripts/dev-stack.sh should be executable"
        );
    }

    let script = std::fs::read_to_string(script_path).unwrap();
    for expected in [
        "start-echo",
        "start-relay",
        "start-punch",
        "start-control",
        "start-agent",
        "start-mobile",
        "check",
        "stop",
        "run-all",
        "QUIC_TEST_PATH",
        "QUIC_TEST_CONTROL_STATE_DB",
        "QUIC_TEST_CONTROL_ADMIN_EMAIL",
        "QUIC_TEST_CONTROL_ADMIN_PASSWORD",
        "--bootstrap-admin-email",
        "--bootstrap-admin-password",
        "QUIC_TEST_RELAY_CONTROL_REGISTER",
        "--control-url",
        "--control-token",
        "--relay-id",
        "--advertise-addr",
        "--debug-admin-listen",
        "--capacity-streams",
        "--heartbeat-interval-sec",
        "fallback",
        "nohup",
        "assert_http_forward_response",
        "assert_admin_session_visible",
        "quic-test-forward-ok",
        "wait_for_tcp",
        "mobile-cli forwarding",
        "stop_port_owner_if_expected",
        "lsof -nP -iTCP",
        "mobile-cl",
        "Relay Debug Admin",
        "Control Admin",
        "admin-token",
        "relay-token",
        "admin-users",
        "admin-usage",
        "admin-devices",
        "admin-relays",
        "admin-audit",
        "admin-device-access",
        "admin-create-user",
        "admin-grant-device-access",
        "admin-revoke-device-access",
        "mobile-cli admin users",
        "mobile-cli admin usage",
        "mobile-cli admin device-access",
        "mobile-cli admin create-user",
        "mobile-cli admin grant-device-access",
        "--print-admin-token",
        "--print-relay-token",
        "QUIC_TEST_ADMIN_SUBJECT",
        "Ctrl-C",
        "--state-db",
    ] {
        assert!(script.contains(expected), "missing {expected}");
    }
}

#[test]
fn production_check_documents_release_readiness() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let script_path = workspace.join("scripts/production-check.sh");
    let relay_installer_path = workspace.join("scripts/install-relayd.sh");
    let checklist_path = workspace.join("docs/production-readiness.md");
    let mobile_acceptance_path = workspace.join("docs/mobile-device-acceptance.md");

    let script_metadata =
        std::fs::metadata(&script_path).expect("scripts/production-check.sh should exist");
    assert!(script_metadata.is_file());
    let relay_installer_metadata =
        std::fs::metadata(&relay_installer_path).expect("scripts/install-relayd.sh should exist");
    assert!(relay_installer_metadata.is_file());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        assert_ne!(
            script_metadata.permissions().mode() & 0o111,
            0,
            "scripts/production-check.sh should be executable"
        );
        assert_ne!(
            relay_installer_metadata.permissions().mode() & 0o111,
            0,
            "scripts/install-relayd.sh should be executable"
        );
    }

    let checklist_metadata =
        std::fs::metadata(&checklist_path).expect("docs/production-readiness.md should exist");
    assert!(checklist_metadata.is_file());
    let mobile_acceptance_metadata = std::fs::metadata(&mobile_acceptance_path)
        .expect("docs/mobile-device-acceptance.md should exist");
    assert!(mobile_acceptance_metadata.is_file());

    let script = std::fs::read_to_string(script_path).unwrap();
    for expected in [
        "cargo fmt --check",
        "cargo test --workspace --no-run",
        "scripts/gen-mobile-bindings.sh --language all",
        "cargo test -p mobilecode_connect_mobile_core --lib mobile_grant_",
        "cargo test -p mobilecode_connect_mobile_core --test mobile_platform_wrappers",
        "cargo test -p mobilecode_connect_sdk --test live_workflow",
        "cargo test -p mobilecode_connect_mobile_core --test smoke_script",
        "bash -n scripts/dev-stack.sh",
        "bash -n scripts/e2e-smoke.sh",
        "bash -n scripts/package-mobile-ios.sh",
        "bash -n scripts/package-mobile-android.sh",
        "bash -n scripts/install-relayd.sh",
        "scripts/package-mobile-ios.sh --dry-run",
        "--ios-min-version 17.0",
        "aarch64-apple-ios,aarch64-apple-ios-sim,x86_64-apple-ios",
        "--xcframework-output target/mobile-package-dry-run/ios/mobilecode_connect_mobile_coreFFI.xcframework",
        "scripts/package-mobile-android.sh --dry-run",
        "--gradle-task assembleRelease",
        "--aar-output-dir target/mobile-package-dry-run/android/aar",
        "QUIC_PROD_CHECK_MOBILE_PACKAGE",
        "QUIC_PROD_CHECK_IOS_PACKAGE",
        "QUIC_PROD_CHECK_ANDROID_PACKAGE",
        "QUIC_PROD_CHECK_DEVICE_SIGNOFF",
        "docs/mobile-device-acceptance.md",
        "QUIC_PROD_CHECK_FULL",
        "QUIC_PROD_CHECK_E2E",
        "QUIC_TUNNEL_STRICT_AUTH",
        "QUIC_TEST_TOKEN_SECRET",
        "./scripts/e2e-smoke.sh",
        "production-readiness.md",
    ] {
        assert!(script.contains(expected), "missing {expected}");
    }

    let checklist = std::fs::read_to_string(checklist_path).unwrap();
    for expected in [
        "Production Readiness",
        "Release Gate",
        "Security",
        "Authentication",
        "Secrets",
        "TLS",
        "Persistence",
        "Mobile",
        "Relay Bootstrap",
        "Control-owned Relay management",
        "Control-owned Relay Live Ops",
        "per-session snapshots",
        "relay-scoped commands",
        "Production browsers must not connect directly to Relay Admin HTTP",
        "scripts/install-relayd.sh",
        "--no-service",
        "--debug-admin-listen",
        "debug-only local Relay admin",
        "create-relay-bootstrap",
        "Keychain",
        "Android Keystore",
        "MobileCodeConnectMobileGrantSecureStore",
        "QUIC_PROD_CHECK_MOBILE_PACKAGE=1",
        "QUIC_PROD_CHECK_DEVICE_SIGNOFF=1",
        "mobile-device-acceptance.md",
        "Operations",
        "Observability",
        "Billing",
        "Quota",
        "Backup",
        "Rollback",
        "scripts/production-check.sh",
        "QUIC_TUNNEL_STRICT_AUTH=true",
        "dev-secret",
    ] {
        assert!(checklist.contains(expected), "missing {expected}");
    }
    assert!(!checklist.contains("--admin-listen 127.0.0.1:9090"));

    let mobile_acceptance = std::fs::read_to_string(mobile_acceptance_path).unwrap();
    for expected in [
        "Mobile Device Acceptance",
        "iOS",
        "Android",
        "Keychain",
        "Android Keystore",
        "WebView",
        "browser proxy",
        "P2P",
        "Relay",
        "revoke",
        "direct fallback",
        "public IP",
        "LocalNetworkAndDomain",
        "QUIC_PROD_CHECK_DEVICE_SIGNOFF",
    ] {
        assert!(mobile_acceptance.contains(expected), "missing {expected}");
    }
}

#[test]
fn relay_installer_no_service_dry_run_skips_systemd_and_prints_manual_start() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let relay_installer_path = workspace.join("scripts/install-relayd.sh");

    let output = Command::new("bash")
        .arg(&relay_installer_path)
        .arg("--dry-run")
        .arg("--no-service")
        .arg("--control-url")
        .arg("127.0.0.1:4242")
        .arg("--bootstrap-id")
        .arg("rb_001")
        .arg("--bootstrap-token")
        .arg("shown-once")
        .arg("--relayd-url")
        .arg("127.0.0.1:4242/relayd")
        .output()
        .expect("install-relayd dry-run should execute");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "install-relayd --no-service dry-run failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(stdout
        .contains("dry-run: exchange POST http://127.0.0.1:4242/relay-bootstraps/rb_001/exchange"));
    assert!(stdout.contains("dry-run: download relayd from http://127.0.0.1:4242/relayd"));
    assert!(stdout.contains("dry-run: no-service mode"));
    assert!(stdout.contains("manual relayd command"));
    assert!(!stdout.contains("--admin-listen"));
    assert!(!stdout.contains("--debug-admin-listen"));
    assert!(!stdout.contains("QUIC_TUNNEL_RELAY_ADMIN_LISTEN"));
    assert!(!stdout.contains("QUIC_TUNNEL_RELAY_ADVERTISE_ADMIN_ADDR"));
    assert!(!stdout.contains("write systemd unit"));
    assert!(!stdout.contains("systemctl enable"));
}

#[test]
fn relay_installer_debug_admin_is_explicit_opt_in() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let relay_installer_path = workspace.join("scripts/install-relayd.sh");

    let output = Command::new("bash")
        .arg(&relay_installer_path)
        .arg("--dry-run")
        .arg("--no-service")
        .arg("--control-url")
        .arg("127.0.0.1:4242")
        .arg("--bootstrap-id")
        .arg("rb_001")
        .arg("--bootstrap-token")
        .arg("shown-once")
        .arg("--relayd-url")
        .arg("127.0.0.1:4242/relayd")
        .arg("--debug-admin-listen")
        .arg("127.0.0.1:9090")
        .output()
        .expect("install-relayd dry-run should execute");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "install-relayd debug admin dry-run failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(stdout.contains("--debug-admin-listen"));
    assert!(stdout.contains("127.0.0.1:9090"));
}

#[test]
fn oauth_server_login_docs_are_current() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let readme = std::fs::read_to_string(workspace.join("README.md")).unwrap();
    let production =
        std::fs::read_to_string(workspace.join("docs/production-readiness.md")).unwrap();
    let combined = format!("{readme}\n{production}");

    for expected in [
        "QUIC_TUNNEL_PUBLIC_URL",
        "QUIC_TUNNEL_GITHUB_CLIENT_ID",
        "QUIC_TUNNEL_GITHUB_CLIENT_SECRET",
        "QUIC_TUNNEL_GITHUB_REDIRECT_URL",
        "system curl",
        "agentd login",
        "agentd login --device-code",
        "agentd run --credential-file",
        "server credential",
        "ControlRole::Agent",
        "Agent credential",
        "/auth/password",
        "auth.password.set",
        "/oauth/identities",
        "oauth_identity.unlink",
        "/server-auth/browser/start",
        "/server-auth/device/start",
        "/server-credentials",
    ] {
        assert!(combined.contains(expected), "missing {expected}");
    }
}

#[test]
fn control_admin_page_targets_current_control_api() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let page_path = workspace.join("docs/control-admin.html");

    let metadata = std::fs::metadata(&page_path).expect("docs/control-admin.html should exist");
    assert!(metadata.is_file());

    let page = std::fs::read_to_string(page_path).unwrap();
    for expected in [
        "Control Admin",
        r#"id="currentIdentity""#,
        r#"id="clearUserToken""#,
        r#"id="clearAdminToken""#,
        r#"id="copyUserToAdmin""#,
        "requireToken(kind)",
        r#"id="listQuery""#,
        r#"id="listSort""#,
        r#"id="listLimit""#,
        r#"id="listOffset""#,
        "pageItems(page)",
        "listPath(path",
        r#"id="dashboardPanel""#,
        r#"id="dashboardMetricsTable""#,
        r#"id="dashboardAuditTable""#,
        r#"id="loadDashboard""#,
        r#"id="operationsPanel""#,
        r#"id="operationLogTable""#,
        r#"id="clearOperationLog""#,
        "recordOperation(",
        "refreshAdminOverview(",
        "runAdminMutation(",
        r#"id="usersPanel""#,
        r#"id="userDetailTable""#,
        r#"id="auditLogsPanel""#,
        r#"id="auditLogsTable""#,
        r#"id="usagePanel""#,
        r#"id="usageTable""#,
        r#"id="usageSort""#,
        r#"id="usageLimit""#,
        r#"id="usageOffset""#,
        r#"id="usageResetUserId""#,
        r#"id="resetUsagePeriod""#,
        r#"id="sessionsPanel""#,
        r#"id="sessionsTable""#,
        r#"id="adminSessionId""#,
        r#"id="closeAdminSession""#,
        "relay_quota_granted_bytes",
        "session_count",
        "actual_uplink_bytes",
        "actual_downlink_bytes",
        "actual_total_bytes",
        r#"id="managedRole""#,
        r#"id="createManagedUser""#,
        r#"id="relayPool""#,
        r#"id="loadRelaySessions""#,
        r#"id="relaySessionsTable""#,
        r#"id="relayCredentialsTable""#,
        r#"id="rotateRelayCredential""#,
        "data-select-relay-id",
        "data-disconnect-relay-session-id",
        "data-select-relay-credential-id",
        "renderRelayRows(relays)",
        "renderRelaySessionRows(sessions)",
        "renderRelayCredentialRows(credentials)",
        "selectRelay(relay)",
        "selectRelayCredential(credential)",
        r#"id="planEditor""#,
        r#"id="planCatalog""#,
        r#"id="planCatalogTable""#,
        "data-select-user-id",
        "data-select-plan-id",
        "setLinkedUserInputs(userId)",
        "selectManagedUser(user)",
        "renderUserRows(users)",
        "renderPlanCatalogRows(plans)",
        r#"id="deviceDetailTable""#,
        r#"id="deviceAccessUserId""#,
        r#"id="deviceAccessTable""#,
        r#"id="grantDeviceAccess""#,
        r#"id="revokeDeviceAccess""#,
        "data-select-device-id",
        "data-select-device-access-user-id",
        "renderDeviceRows(devices)",
        "renderDeviceAccessRows(grants)",
        "selectControlledDevice(device)",
        "selectDeviceAccessGrant(grant)",
        "loadSelectedDeviceContext(",
        "/auth/login",
        "/auth/register",
        "/dashboard",
        "/audit-logs",
        "loadDashboard",
        "/usage/users",
        "/reset",
        "/sessions",
        "/close",
        "/users",
        "/status",
        "/role",
        "/controllers/register",
        "/plans/current",
        "/plans/catalog",
        "/plans/users/",
        "/assign",
        "/relays/register",
        "/sessions/${encodeURIComponent(sessionId)}/disconnect",
        "/relay-credentials",
        "/rotate",
        "/mobile/devices/",
        "/access",
        "last_seen_epoch_sec",
        "admin_addr: \"\"",
        "controlTokenRole",
        "window.localStorage",
        "controlAdmin.adminToken",
    ] {
        assert!(page.contains(expected), "missing {expected}");
    }

    for deprecated in [r#"id="relayAdminAddr""#, "Admin address"] {
        assert!(!page.contains(deprecated), "deprecated {deprecated}");
    }
}

#[test]
fn relay_admin_page_targets_current_admin_api() {
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let page_path = workspace.join("docs/relay-admin.html");

    let metadata = std::fs::metadata(&page_path).expect("docs/relay-admin.html should exist");
    assert!(metadata.is_file());

    let page = std::fs::read_to_string(page_path).unwrap();
    for expected in [
        r#"id="sessionList""#,
        r#"id="refreshSessions""#,
        r#"value="http://127.0.0.1:9090""#,
        "/admin/sessions",
        "function sessionsURL()",
        "function sessionURL(sessionId)",
        "function renderSessionList",
        "function renderSessionDetail",
        "window.location.origin",
        "uplink_bytes",
        "active_streams",
        "duration_sec",
    ] {
        assert!(page.contains(expected), "missing {expected}");
    }
}
