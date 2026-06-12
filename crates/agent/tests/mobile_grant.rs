use mobilecode_connect_agent::mobile_grant::{
    CreateMobileInviteRequest, MobileGrantManager, MobileGrantManagerError,
};
use mobilecode_connect_protocol::{ClientId, DeviceId, MobilePairingRequest, ServiceId};

#[test]
fn mobile_grant_manager_approves_verifies_and_revokes_grants() {
    let manager = MobileGrantManager::default();
    let device_id = DeviceId::new("pc_001");
    let service_id = ServiceId::new("svc_web_3000");
    let client_id = ClientId::new("mobile_001");
    let invite = manager
        .create_invite(
            CreateMobileInviteRequest {
                control_url: "https://control.example.test".to_string(),
                device_id: device_id.clone(),
                allowed_services: vec![service_id.clone()],
                ttl_sec: 60,
                max_uses: 1,
                agent_p2p_cert_fingerprint: Some("sha256:test".to_string()),
            },
            1_000,
        )
        .unwrap();
    let proof = MobilePairingRequest::proof_for(
        device_id.clone(),
        invite.invite_id.clone(),
        client_id.clone(),
        vec![service_id.clone()],
        "pairing-nonce".to_string(),
        &invite.invite_secret,
    )
    .unwrap();
    let pairing = MobilePairingRequest {
        device_id: device_id.clone(),
        invite_id: invite.invite_id.clone(),
        client_id: client_id.clone(),
        requested_services: vec![service_id.clone()],
        nonce: "pairing-nonce".to_string(),
        proof,
    };

    let grant = manager.approve_pairing(&pairing, 1_001).unwrap();
    assert_eq!(grant.device_id, device_id);
    assert_eq!(grant.client_id, client_id);
    assert_eq!(grant.allowed_services, vec![service_id.clone()]);
    assert_eq!(grant.revocation_version, 1);

    let session = grant
        .sign_session_request(service_id.clone(), "session-nonce".to_string())
        .unwrap();
    assert!(manager.verify_session(&session, 1_002).is_ok());

    manager.revoke_grant(&grant.grant_id).unwrap();
    assert_eq!(
        manager.verify_session(&session, 1_003).unwrap_err(),
        MobileGrantManagerError::GrantRevoked
    );
}

#[test]
fn mobile_grant_manager_rejects_bad_or_out_of_scope_pairing() {
    let manager = MobileGrantManager::default();
    let device_id = DeviceId::new("pc_001");
    let invite = manager
        .create_invite(
            CreateMobileInviteRequest {
                control_url: "https://control.example.test".to_string(),
                device_id: device_id.clone(),
                allowed_services: vec![ServiceId::new("svc_web_3000")],
                ttl_sec: 60,
                max_uses: 1,
                agent_p2p_cert_fingerprint: None,
            },
            1_000,
        )
        .unwrap();
    let proof = MobilePairingRequest::proof_for(
        device_id.clone(),
        invite.invite_id.clone(),
        ClientId::new("mobile_001"),
        vec![ServiceId::new("svc_ssh")],
        "pairing-nonce".to_string(),
        &invite.invite_secret,
    )
    .unwrap();
    let pairing = MobilePairingRequest {
        device_id,
        invite_id: invite.invite_id,
        client_id: ClientId::new("mobile_001"),
        requested_services: vec![ServiceId::new("svc_ssh")],
        nonce: "pairing-nonce".to_string(),
        proof,
    };

    assert_eq!(
        manager.approve_pairing(&pairing, 1_001).unwrap_err(),
        MobileGrantManagerError::ScopeDenied
    );
}

#[test]
fn mobile_grant_manager_persists_grants_and_revocations() {
    let path = unique_temp_path("mobile-grants.json");
    let device_id = DeviceId::new("pc_001");
    let service_id = ServiceId::new("svc_web_3000");
    let client_id = ClientId::new("mobile_001");

    let manager = MobileGrantManager::load_or_create_file(&path).unwrap();
    let invite = manager
        .create_invite(
            CreateMobileInviteRequest {
                control_url: "https://control.example.test".to_string(),
                device_id: device_id.clone(),
                allowed_services: vec![service_id.clone()],
                ttl_sec: 60,
                max_uses: 1,
                agent_p2p_cert_fingerprint: None,
            },
            1_000,
        )
        .unwrap();
    let proof = MobilePairingRequest::proof_for(
        device_id.clone(),
        invite.invite_id.clone(),
        client_id.clone(),
        vec![service_id.clone()],
        "pairing-nonce".to_string(),
        &invite.invite_secret,
    )
    .unwrap();
    let pairing = MobilePairingRequest {
        device_id,
        invite_id: invite.invite_id,
        client_id,
        requested_services: vec![service_id.clone()],
        nonce: "pairing-nonce".to_string(),
        proof,
    };
    let grant = manager.approve_pairing(&pairing, 1_001).unwrap();
    let session = grant
        .sign_session_request(service_id, "session-nonce".to_string())
        .unwrap();

    let restored = MobileGrantManager::load_or_create_file(&path).unwrap();
    assert!(restored.verify_session(&session, 1_002).is_ok());
    restored.revoke_grant(&grant.grant_id).unwrap();

    let restored_again = MobileGrantManager::load_or_create_file(&path).unwrap();
    assert_eq!(
        restored_again.verify_session(&session, 1_003).unwrap_err(),
        MobileGrantManagerError::GrantRevoked
    );

    std::fs::remove_file(path).unwrap();
}

#[test]
fn mobile_grant_manager_lists_and_revokes_invites_and_grants() {
    let manager = MobileGrantManager::default();
    let device_id = DeviceId::new("pc_001");
    let service_id = ServiceId::new("svc_web_3000");
    let client_id = ClientId::new("mobile_001");
    let invite = manager
        .create_invite(
            CreateMobileInviteRequest {
                control_url: "https://control.example.test".to_string(),
                device_id: device_id.clone(),
                allowed_services: vec![service_id.clone()],
                ttl_sec: 60,
                max_uses: 2,
                agent_p2p_cert_fingerprint: Some("cert-fp".to_string()),
            },
            1_000,
        )
        .unwrap();
    let pairing = pairing_request(
        &invite,
        device_id.clone(),
        client_id.clone(),
        vec![service_id.clone()],
        "pairing-nonce",
    );
    let grant = manager.approve_pairing(&pairing, 1_001).unwrap();

    let invites = manager.list_invites();
    assert_eq!(invites.len(), 1);
    assert_eq!(invites[0].invite_id, invite.invite_id);
    assert_eq!(invites[0].uses, 1);
    assert_eq!(invites[0].max_uses, 2);
    assert!(!invites[0].revoked);
    assert_eq!(
        invites[0].agent_p2p_cert_fingerprint.as_deref(),
        Some("cert-fp")
    );

    let grants = manager.list_grants();
    assert_eq!(grants.len(), 1);
    assert_eq!(grants[0].grant_id, grant.grant_id);
    assert_eq!(grants[0].client_id, client_id);
    assert_eq!(grants[0].revocation_version, 1);
    assert!(grants[0].enabled);
    assert_eq!(
        grants[0].agent_p2p_cert_fingerprint.as_deref(),
        Some("cert-fp")
    );

    manager.revoke_invite(&invite.invite_id).unwrap();
    assert!(manager.list_invites()[0].revoked);
    manager.revoke_grant(&grant.grant_id).unwrap();
    assert!(!manager.list_grants()[0].enabled);
}

#[test]
fn mobile_grant_manager_reuses_grant_for_identical_pairing_retry() {
    let manager = MobileGrantManager::default();
    let device_id = DeviceId::new("pc_001");
    let service_id = ServiceId::new("svc_web_3000");
    let client_id = ClientId::new("mobile_001");
    let invite = invite(&manager, device_id.clone(), service_id.clone(), 1);
    let pairing = pairing_request(
        &invite,
        device_id,
        client_id,
        vec![service_id],
        "pairing-nonce",
    );

    let first = manager.approve_pairing(&pairing, 1_001).unwrap();
    let retry = manager.approve_pairing(&pairing, 1_002).unwrap();

    assert_eq!(retry.grant_id, first.grant_id);
    assert_eq!(manager.list_invites()[0].uses, 1);
}

#[test]
fn mobile_grant_manager_rejects_reused_pairing_nonce_for_different_payload() {
    let manager = MobileGrantManager::default();
    let device_id = DeviceId::new("pc_001");
    let web = ServiceId::new("svc_web_3000");
    let ssh = ServiceId::new("svc_ssh");
    let client_id = ClientId::new("mobile_001");
    let invite = manager
        .create_invite(
            CreateMobileInviteRequest {
                control_url: "https://control.example.test".to_string(),
                device_id: device_id.clone(),
                allowed_services: vec![web.clone(), ssh.clone()],
                ttl_sec: 60,
                max_uses: 2,
                agent_p2p_cert_fingerprint: None,
            },
            1_000,
        )
        .unwrap();
    let first = pairing_request(
        &invite,
        device_id.clone(),
        client_id.clone(),
        vec![web],
        "pairing-nonce",
    );
    let replay = pairing_request(&invite, device_id, client_id, vec![ssh], "pairing-nonce");

    manager.approve_pairing(&first, 1_001).unwrap();
    assert_eq!(
        manager.approve_pairing(&replay, 1_002).unwrap_err(),
        MobileGrantManagerError::ReplayDetected
    );
}

#[test]
fn mobile_grant_manager_rejects_reused_session_nonce_for_different_payload() {
    let manager = MobileGrantManager::default();
    let device_id = DeviceId::new("pc_001");
    let web = ServiceId::new("svc_web_3000");
    let ssh = ServiceId::new("svc_ssh");
    let client_id = ClientId::new("mobile_001");
    let invite = manager
        .create_invite(
            CreateMobileInviteRequest {
                control_url: "https://control.example.test".to_string(),
                device_id: device_id.clone(),
                allowed_services: vec![web.clone(), ssh.clone()],
                ttl_sec: 60,
                max_uses: 1,
                agent_p2p_cert_fingerprint: None,
            },
            1_000,
        )
        .unwrap();
    let grant = manager
        .approve_pairing(
            &pairing_request(
                &invite,
                device_id,
                client_id,
                vec![web.clone(), ssh.clone()],
                "pairing-nonce",
            ),
            1_001,
        )
        .unwrap();
    let first = grant
        .sign_session_request(web, "session-nonce".to_string())
        .unwrap();
    let replay = grant
        .sign_session_request(ssh, "session-nonce".to_string())
        .unwrap();

    manager.verify_session(&first, 1_002).unwrap();
    assert_eq!(
        manager.verify_session(&first, 1_003).unwrap().grant_id,
        grant.grant_id
    );
    assert_eq!(
        manager.verify_session(&replay, 1_004).unwrap_err(),
        MobileGrantManagerError::ReplayDetected
    );
}

fn invite(
    manager: &MobileGrantManager,
    device_id: DeviceId,
    service_id: ServiceId,
    max_uses: u32,
) -> mobilecode_connect_protocol::MobileInvitePayload {
    manager
        .create_invite(
            CreateMobileInviteRequest {
                control_url: "https://control.example.test".to_string(),
                device_id,
                allowed_services: vec![service_id],
                ttl_sec: 60,
                max_uses,
                agent_p2p_cert_fingerprint: None,
            },
            1_000,
        )
        .unwrap()
}

fn pairing_request(
    invite: &mobilecode_connect_protocol::MobileInvitePayload,
    device_id: DeviceId,
    client_id: ClientId,
    requested_services: Vec<ServiceId>,
    nonce: &str,
) -> MobilePairingRequest {
    let nonce = nonce.to_string();
    let proof = MobilePairingRequest::proof_for(
        device_id.clone(),
        invite.invite_id.clone(),
        client_id.clone(),
        requested_services.clone(),
        nonce.clone(),
        &invite.invite_secret,
    )
    .unwrap();
    MobilePairingRequest {
        device_id,
        invite_id: invite.invite_id.clone(),
        client_id,
        requested_services,
        nonce,
        proof,
    }
}

fn unique_temp_path(name: &str) -> std::path::PathBuf {
    static NEXT_TEMP_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let id = NEXT_TEMP_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    std::env::temp_dir().join(format!("quic-test-agent-mobile-grant-{suffix}-{id}-{name}"))
}
