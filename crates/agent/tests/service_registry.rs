use mobilecode_connect_agent::{
    config::ServiceConfig,
    service_registry::{ServiceRegistry, ServiceRegistryError},
};
use mobilecode_connect_protocol::{ServiceId, ServiceProtocol};

fn service(id: &str, port: u16) -> ServiceConfig {
    ServiceConfig {
        service_id: ServiceId::new(id),
        name: "Dev Web".to_string(),
        protocol: ServiceProtocol::Tcp,
        target_host: "127.0.0.1".to_string(),
        target_port: port,
    }
}

#[test]
fn registry_returns_service_target_by_id() {
    let registry = ServiceRegistry::new(vec![service("svc_web_3000", 3000)]).unwrap();
    let found = registry.get(&ServiceId::new("svc_web_3000")).unwrap();

    assert_eq!(found.target_addr(), "127.0.0.1:3000");
}

#[test]
fn registry_rejects_duplicate_service_id() {
    let err = ServiceRegistry::new(vec![
        service("svc_web_3000", 3000),
        service("svc_web_3000", 3001),
    ])
    .unwrap_err();

    assert!(matches!(
        err,
        ServiceRegistryError::DuplicateServiceId { .. }
    ));
}

#[test]
fn registry_rejects_empty_target_host() {
    let mut config = service("svc_web_3000", 3000);
    config.target_host.clear();

    let err = ServiceRegistry::new(vec![config]).unwrap_err();

    assert!(matches!(err, ServiceRegistryError::InvalidTarget { .. }));
}

#[test]
fn registry_rejects_zero_target_port() {
    let err = ServiceRegistry::new(vec![service("svc_web_3000", 0)]).unwrap_err();

    assert!(matches!(err, ServiceRegistryError::InvalidTarget { .. }));
}

#[test]
fn registry_allows_local_lan_and_domain_targets() {
    for target_host in [
        "localhost",
        "127.0.0.1",
        "192.168.1.20",
        "10.0.0.5",
        "172.16.4.8",
        "printer.home.arpa",
        "nas.local",
    ] {
        let mut config = service("svc_web_3000", 3000);
        config.target_host = target_host.to_string();

        ServiceRegistry::new(vec![config]).expect(target_host);
    }
}

#[test]
fn registry_rejects_public_or_unspecified_ip_targets() {
    for target_host in ["0.0.0.0", "8.8.8.8", "1.1.1.1", "255.255.255.255"] {
        let mut config = service("svc_web_3000", 3000);
        config.target_host = target_host.to_string();

        let err = ServiceRegistry::new(vec![config]).unwrap_err();
        assert!(matches!(err, ServiceRegistryError::InvalidTarget { .. }));
    }
}
