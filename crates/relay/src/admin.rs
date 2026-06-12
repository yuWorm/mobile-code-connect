use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Html,
    routing::{get, post},
    Json, Router,
};
use mobilecode_connect_protocol::{RelayLimits, SessionId, TrafficStats};
use serde::{Deserialize, Serialize};

use crate::runtime::RelayConnectionStore;
use crate::session::{RelaySession, RelaySessionMetrics, RelaySessionState, RelaySessionStore};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayAdminSession {
    pub session_id: SessionId,
    pub state: String,
    pub mobile_bound: bool,
    pub agent_bound: bool,
    pub limits: RelayLimits,
    pub stats: TrafficStats,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayAdminHealth {
    pub status: String,
    pub active_sessions: u64,
    pub active_streams: u64,
    pub total_uplink_bytes: u64,
    pub total_downlink_bytes: u64,
    pub total_bytes: u64,
}

pub fn routes(session_store: RelaySessionStore) -> Router {
    routes_for_state(RelayAdminState {
        sessions: session_store,
        connections: None,
    })
}

pub(crate) fn routes_with_connections(
    session_store: RelaySessionStore,
    connection_store: RelayConnectionStore,
) -> Router {
    routes_for_state(RelayAdminState {
        sessions: session_store,
        connections: Some(connection_store),
    })
}

fn routes_for_state(state: RelayAdminState) -> Router {
    Router::new()
        .route("/admin", get(admin_page))
        .route("/admin/", get(admin_page))
        .route("/admin/health", get(health))
        .route("/admin/sessions", get(list_sessions))
        .route("/admin/sessions/{session_id}", get(get_session))
        .route(
            "/admin/sessions/{session_id}/disconnect",
            post(disconnect_session),
        )
        .with_state(state)
}

#[derive(Clone)]
struct RelayAdminState {
    sessions: RelaySessionStore,
    connections: Option<RelayConnectionStore>,
}

async fn admin_page() -> Html<&'static str> {
    Html(include_str!("../../../docs/relay-admin.html"))
}

async fn health(State(state): State<RelayAdminState>) -> Json<RelayAdminHealth> {
    Json(RelayAdminHealth::from(state.sessions.metrics()))
}

async fn list_sessions(State(state): State<RelayAdminState>) -> Json<Vec<RelayAdminSession>> {
    Json(
        state
            .sessions
            .list()
            .into_iter()
            .map(RelayAdminSession::from)
            .collect(),
    )
}

async fn get_session(
    State(state): State<RelayAdminState>,
    Path(session_id): Path<String>,
) -> Result<Json<RelayAdminSession>, StatusCode> {
    state
        .sessions
        .get(&SessionId::new(session_id))
        .map(RelayAdminSession::from)
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

async fn disconnect_session(
    State(state): State<RelayAdminState>,
    Path(session_id): Path<String>,
) -> Result<Json<RelayAdminSession>, StatusCode> {
    let session_id = SessionId::new(session_id);
    let session = state
        .sessions
        .close(&session_id)
        .map(RelayAdminSession::from)
        .map_err(|error| match error {
            crate::session::RelayError::SessionNotFound { .. } => StatusCode::NOT_FOUND,
            _ => StatusCode::CONFLICT,
        })?;
    if let Some(connections) = &state.connections {
        connections.close_session(&session_id);
    }
    Ok(Json(session))
}

impl From<RelaySessionMetrics> for RelayAdminHealth {
    fn from(metrics: RelaySessionMetrics) -> Self {
        Self {
            status: "healthy".to_string(),
            active_sessions: metrics.active_sessions,
            active_streams: metrics.active_streams,
            total_uplink_bytes: metrics.total_uplink_bytes,
            total_downlink_bytes: metrics.total_downlink_bytes,
            total_bytes: metrics.total_bytes,
        }
    }
}

impl From<RelaySession> for RelayAdminSession {
    fn from(session: RelaySession) -> Self {
        Self {
            session_id: session.session_id,
            state: session_state(&session.state).to_string(),
            mobile_bound: session.mobile.is_some(),
            agent_bound: session.agent.is_some(),
            limits: session.limits,
            stats: session.stats,
        }
    }
}

fn session_state(state: &RelaySessionState) -> &'static str {
    match state {
        RelaySessionState::Waiting => "waiting",
        RelaySessionState::Ready => "ready",
        RelaySessionState::Closed => "closed",
    }
}
