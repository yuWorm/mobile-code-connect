use std::{
    future::Future,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use mobilecode_connect_protocol::ServiceId;
use mobilecode_connect_tunnel::stream::{read_data_header, TunnelStreamError};
use tokio::{
    io::{self, AsyncRead, AsyncWrite},
    net::{lookup_host, TcpStream},
};

use crate::service_registry::ServiceRegistry;

pub async fn handle_data_stream<S>(
    stream: S,
    registry: ServiceRegistry,
) -> Result<(), AgentStreamError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    handle_data_stream_with_policy(stream, registry, TargetAccessPolicy::default()).await
}

pub async fn handle_data_stream_with_policy<S>(
    stream: S,
    registry: ServiceRegistry,
    policy: TargetAccessPolicy,
) -> Result<(), AgentStreamError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    handle_data_stream_with_policy_and_resolver(stream, registry, policy, resolve_target_addrs)
        .await
}

pub async fn handle_data_stream_with_policy_and_resolver<S, R, Fut>(
    mut stream: S,
    registry: ServiceRegistry,
    policy: TargetAccessPolicy,
    resolver: R,
) -> Result<(), AgentStreamError>
where
    S: AsyncRead + AsyncWrite + Unpin,
    R: Fn(String, u16) -> Fut,
    Fut: Future<Output = Result<Vec<SocketAddr>, std::io::Error>>,
{
    let header = read_data_header(&mut stream).await?;
    let service =
        registry
            .get(&header.service_id)
            .ok_or_else(|| AgentStreamError::ServiceNotFound {
                service_id: header.service_id.clone(),
            })?;
    let target_addrs = resolver(service.target_host.clone(), service.target_port)
        .await
        .map_err(|source| AgentStreamError::TargetResolveFailed {
            service_id: header.service_id.clone(),
            target_host: service.target_host.clone(),
            source,
        })?;
    let allowed_addrs = policy
        .allowed_target_addrs(&service.target_host, target_addrs)
        .map_err(|_| AgentStreamError::TargetAccessDenied {
            service_id: header.service_id.clone(),
            target_host: service.target_host.clone(),
        })?;
    let target_addr =
        allowed_addrs
            .into_iter()
            .next()
            .ok_or_else(|| AgentStreamError::TargetAccessDenied {
                service_id: header.service_id.clone(),
                target_host: service.target_host.clone(),
            })?;
    let mut target = TcpStream::connect(target_addr).await.map_err(|source| {
        AgentStreamError::ServiceDialFailed {
            service_id: header.service_id,
            target_addr: target_addr.to_string(),
            source,
        }
    })?;

    io::copy_bidirectional(&mut stream, &mut target).await?;
    Ok(())
}

async fn resolve_target_addrs(
    target_host: String,
    target_port: u16,
) -> Result<Vec<SocketAddr>, std::io::Error> {
    Ok(lookup_host((target_host.as_str(), target_port))
        .await?
        .collect())
}

#[derive(Debug, Clone)]
pub struct TargetAccessPolicy {
    allowed_lan_cidrs: Vec<IpNetwork>,
    allow_domains: bool,
}

impl Default for TargetAccessPolicy {
    fn default() -> Self {
        Self {
            allowed_lan_cidrs: detected_allowed_lan_cidrs(),
            allow_domains: true,
        }
    }
}

impl TargetAccessPolicy {
    pub fn with_allowed_lan_cidrs<I, S>(cidrs: I) -> Result<Self, TargetAccessPolicyError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Ok(Self {
            allowed_lan_cidrs: cidrs
                .into_iter()
                .map(|cidr| cidr.as_ref().parse())
                .collect::<Result<Vec<_>, _>>()?,
            allow_domains: true,
        })
    }

    fn allowed_target_addrs(
        &self,
        target_host: &str,
        target_addrs: Vec<SocketAddr>,
    ) -> Result<Vec<SocketAddr>, TargetAccessPolicyError> {
        let host = target_host.trim();
        if let Ok(ip) = host.parse::<IpAddr>() {
            if self.allows_ip(ip) {
                return Ok(target_addrs
                    .into_iter()
                    .filter(|addr| addr.ip() == ip)
                    .collect());
            }
            return Err(TargetAccessPolicyError::Denied);
        }

        if self.allow_domains && is_domain_name(host) {
            let allowed = target_addrs
                .into_iter()
                .filter(|addr| self.allows_ip(addr.ip()))
                .collect::<Vec<_>>();
            if allowed.is_empty() {
                return Err(TargetAccessPolicyError::Denied);
            }
            return Ok(allowed);
        }

        Err(TargetAccessPolicyError::Denied)
    }

    fn allows_ip(&self, ip: IpAddr) -> bool {
        self.allowed_lan_cidrs
            .iter()
            .any(|network| network.contains(ip))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IpNetwork {
    addr: IpAddr,
    prefix_len: u8,
}

impl IpNetwork {
    fn contains(&self, ip: IpAddr) -> bool {
        match (self.addr, ip) {
            (IpAddr::V4(network), IpAddr::V4(ip)) => {
                network_prefix_match(u32::from(network), u32::from(ip), self.prefix_len, 32)
            }
            (IpAddr::V6(network), IpAddr::V6(ip)) => {
                network_prefix_match(u128::from(network), u128::from(ip), self.prefix_len, 128)
            }
            _ => false,
        }
    }
}

impl std::str::FromStr for IpNetwork {
    type Err = TargetAccessPolicyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let (addr, prefix_len) = value
            .split_once('/')
            .ok_or(TargetAccessPolicyError::InvalidCidr)?;
        let addr = addr
            .parse::<IpAddr>()
            .map_err(|_| TargetAccessPolicyError::InvalidCidr)?;
        let prefix_len = prefix_len
            .parse::<u8>()
            .map_err(|_| TargetAccessPolicyError::InvalidCidr)?;
        let max_prefix = match addr {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        if prefix_len > max_prefix {
            return Err(TargetAccessPolicyError::InvalidCidr);
        }
        Ok(Self { addr, prefix_len })
    }
}

fn network_prefix_match<T>(network: T, ip: T, prefix_len: u8, bits: u8) -> bool
where
    T: Copy + From<u8> + std::ops::Not<Output = T> + std::ops::Shl<u8, Output = T>,
    T: std::ops::BitAnd<Output = T> + PartialEq,
{
    if prefix_len == 0 {
        return true;
    }
    let mask = !T::from(0) << (bits - prefix_len);
    (network & mask) == (ip & mask)
}

fn default_allowed_lan_cidrs() -> Vec<IpNetwork> {
    ["127.0.0.0/8", "::1/128"]
        .into_iter()
        .filter_map(|cidr| cidr.parse().ok())
        .collect()
}

fn detected_allowed_lan_cidrs() -> Vec<IpNetwork> {
    let mut networks = default_allowed_lan_cidrs();
    networks.extend(detect_interface_lan_cidrs());
    networks
}

#[cfg(unix)]
fn detect_interface_lan_cidrs() -> Vec<IpNetwork> {
    let mut ifaddrs = std::ptr::null_mut();
    // SAFETY: getifaddrs initializes a linked list owned by libc when it returns 0.
    if unsafe { libc::getifaddrs(&mut ifaddrs) } != 0 {
        return Vec::new();
    }

    let mut networks = Vec::new();
    let mut current = ifaddrs;
    while !current.is_null() {
        // SAFETY: current is a node in the getifaddrs linked list until freeifaddrs.
        let item = unsafe { &*current };
        if !item.ifa_addr.is_null() && !item.ifa_netmask.is_null() {
            if let Some(cidr) = interface_cidr(item.ifa_addr, item.ifa_netmask) {
                if let Ok(network) = cidr.parse() {
                    networks.push(network);
                }
            }
        }
        current = item.ifa_next;
    }

    // SAFETY: ifaddrs was returned by getifaddrs above and has not been freed.
    unsafe { libc::freeifaddrs(ifaddrs) };
    networks
}

#[cfg(not(unix))]
fn detect_interface_lan_cidrs() -> Vec<IpNetwork> {
    Vec::new()
}

#[cfg(unix)]
fn interface_cidr(addr: *const libc::sockaddr, netmask: *const libc::sockaddr) -> Option<String> {
    // SAFETY: caller checks the pointers are non-null.
    let family = unsafe { (*addr).sa_family as i32 };
    if family != libc::AF_INET {
        return None;
    }

    // SAFETY: AF_INET sockaddr pointers can be viewed as sockaddr_in.
    let addr = unsafe { &*(addr as *const libc::sockaddr_in) };
    let netmask = unsafe { &*(netmask as *const libc::sockaddr_in) };
    let ip = Ipv4Addr::from(u32::from_be(addr.sin_addr.s_addr));
    let mask = Ipv4Addr::from(u32::from_be(netmask.sin_addr.s_addr));
    detected_lan_cidr_for_interface(&ip.to_string(), &mask.to_string())
}

pub fn detected_lan_cidr_for_interface(ip: &str, netmask: &str) -> Option<String> {
    let ip = ip.parse::<Ipv4Addr>().ok()?;
    let netmask = netmask.parse::<Ipv4Addr>().ok()?;
    if !is_allowed_detected_lan_ipv4(ip) {
        return None;
    }
    let mask = u32::from(netmask);
    let prefix_len = mask.count_ones() as u8;
    let expected_mask = if prefix_len == 0 {
        0
    } else {
        !0_u32 << (32 - prefix_len)
    };
    if mask != expected_mask {
        return None;
    }
    let network = Ipv4Addr::from(u32::from(ip) & mask);
    Some(format!("{network}/{prefix_len}"))
}

fn is_allowed_detected_lan_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_loopback() || ip.is_private() || ip.is_link_local()
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
pub enum TargetAccessPolicyError {
    #[error("invalid CIDR")]
    InvalidCidr,
    #[error("target access denied")]
    Denied,
}

#[derive(Debug, thiserror::Error)]
pub enum AgentStreamError {
    #[error("service_id not found: {service_id}")]
    ServiceNotFound { service_id: ServiceId },
    #[error("target access denied for {service_id}: {target_host}")]
    TargetAccessDenied {
        service_id: ServiceId,
        target_host: String,
    },
    #[error("failed to resolve target {target_host} for {service_id}: {source}")]
    TargetResolveFailed {
        service_id: ServiceId,
        target_host: String,
        source: std::io::Error,
    },
    #[error("failed to dial target {target_addr} for {service_id}: {source}")]
    ServiceDialFailed {
        service_id: ServiceId,
        target_addr: String,
        source: std::io::Error,
    },
    #[error(transparent)]
    Stream(#[from] TunnelStreamError),
    #[error("stream copy failed: {0}")]
    Io(#[from] std::io::Error),
}
