#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TunnelPath {
    Relay,
    P2p,
}

#[derive(Debug, Default, Clone)]
pub struct PathSelector;

impl PathSelector {
    pub fn select(&self) -> TunnelPath {
        TunnelPath::Relay
    }

    pub fn select_with_probe_result(&self, p2p_ready: bool) -> TunnelPath {
        if p2p_ready {
            TunnelPath::P2p
        } else {
            TunnelPath::Relay
        }
    }
}
