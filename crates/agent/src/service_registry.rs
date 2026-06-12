use std::{collections::HashMap, net::IpAddr};

use mobilecode_connect_protocol::ServiceId;

use crate::config::ServiceConfig;

#[derive(Debug, Clone)]
pub struct ServiceRegistry {
    services: HashMap<ServiceId, ServiceConfig>,
}

impl ServiceRegistry {
    pub fn new(services: Vec<ServiceConfig>) -> Result<Self, ServiceRegistryError> {
        let mut by_id = HashMap::with_capacity(services.len());

        for service in services {
            validate_target(&service)?;
            if by_id
                .insert(service.service_id.clone(), service.clone())
                .is_some()
            {
                return Err(ServiceRegistryError::DuplicateServiceId {
                    service_id: service.service_id,
                });
            }
        }

        Ok(Self { services: by_id })
    }

    pub fn get(&self, service_id: &ServiceId) -> Option<&ServiceConfig> {
        self.services.get(service_id)
    }
}

fn validate_target(service: &ServiceConfig) -> Result<(), ServiceRegistryError> {
    if service.target_host.trim().is_empty()
        || service.target_port == 0
        || !is_allowed_target_host(&service.target_host)
    {
        return Err(ServiceRegistryError::InvalidTarget {
            service_id: service.service_id.clone(),
            target_host: service.target_host.clone(),
            target_port: service.target_port,
        });
    }

    Ok(())
}

fn is_allowed_target_host(target_host: &str) -> bool {
    let host = target_host.trim();
    if let Ok(ip) = host.parse::<IpAddr>() {
        return is_allowed_target_ip(ip);
    }

    is_domain_name(host)
}

fn is_allowed_target_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            ip.is_loopback()
                || ip.is_private()
                || ip.is_link_local()
                || (ip.octets()[0] == 100 && (64..=127).contains(&ip.octets()[1]))
        }
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    }
}

fn is_domain_name(host: &str) -> bool {
    if host.len() > 253 || host.starts_with('.') || host.ends_with('.') {
        return false;
    }

    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

#[derive(Debug, thiserror::Error)]
pub enum ServiceRegistryError {
    #[error("duplicate service_id: {service_id}")]
    DuplicateServiceId { service_id: ServiceId },
    #[error("invalid target for {service_id}: {target_host}:{target_port}")]
    InvalidTarget {
        service_id: ServiceId,
        target_host: String,
        target_port: u16,
    },
}
