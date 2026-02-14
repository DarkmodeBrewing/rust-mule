use axum::{Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

use crate::{
    api::ApiState,
    config::{ApiAuthMode, Config, parse_api_bind_host},
};

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SettingsGeneral {
    pub(crate) log_level: String,
    pub(crate) log_to_file: bool,
    pub(crate) log_file_level: String,
    pub(crate) auto_open_ui: bool,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SettingsSam {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) session_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SettingsApi {
    pub(crate) host: String,
    pub(crate) port: u16,
    pub(crate) enable_debug_endpoints: bool,
    pub(crate) auth_mode: ApiAuthMode,
    pub(crate) rate_limit_enabled: bool,
    pub(crate) rate_limit_window_secs: u64,
    pub(crate) rate_limit_auth_bootstrap_max_per_window: u32,
    pub(crate) rate_limit_session_max_per_window: u32,
    pub(crate) rate_limit_token_rotate_max_per_window: u32,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct SettingsPayload {
    pub(crate) general: SettingsGeneral,
    pub(crate) sam: SettingsSam,
    pub(crate) api: SettingsApi,
}

#[derive(Debug, Serialize)]
pub(crate) struct SettingsResponse {
    pub(crate) settings: SettingsPayload,
    pub(crate) restart_required: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SettingsPatchGeneral {
    #[serde(default)]
    pub(crate) log_level: Option<String>,
    #[serde(default)]
    pub(crate) log_to_file: Option<bool>,
    #[serde(default)]
    pub(crate) log_file_level: Option<String>,
    #[serde(default)]
    pub(crate) auto_open_ui: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SettingsPatchSam {
    #[serde(default)]
    pub(crate) host: Option<String>,
    #[serde(default)]
    pub(crate) port: Option<u16>,
    #[serde(default)]
    pub(crate) session_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SettingsPatchApi {
    #[serde(default)]
    pub(crate) host: Option<String>,
    #[serde(default)]
    pub(crate) port: Option<u16>,
    #[serde(default)]
    pub(crate) enable_debug_endpoints: Option<bool>,
    #[serde(default)]
    pub(crate) auth_mode: Option<ApiAuthMode>,
    #[serde(default)]
    pub(crate) rate_limit_enabled: Option<bool>,
    #[serde(default)]
    pub(crate) rate_limit_window_secs: Option<u64>,
    #[serde(default)]
    pub(crate) rate_limit_auth_bootstrap_max_per_window: Option<u32>,
    #[serde(default)]
    pub(crate) rate_limit_session_max_per_window: Option<u32>,
    #[serde(default)]
    pub(crate) rate_limit_token_rotate_max_per_window: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SettingsPatchRequest {
    #[serde(default)]
    pub(crate) general: Option<SettingsPatchGeneral>,
    #[serde(default)]
    pub(crate) sam: Option<SettingsPatchSam>,
    #[serde(default)]
    pub(crate) api: Option<SettingsPatchApi>,
}

impl SettingsPayload {
    pub(crate) fn from_config(cfg: &Config) -> Self {
        Self {
            general: SettingsGeneral {
                log_level: cfg.general.log_level.clone(),
                log_to_file: cfg.general.log_to_file,
                log_file_level: cfg.general.log_file_level.clone(),
                auto_open_ui: cfg.general.auto_open_ui,
            },
            sam: SettingsSam {
                host: cfg.sam.host.clone(),
                port: cfg.sam.port,
                session_name: cfg.sam.session_name.clone(),
            },
            api: SettingsApi {
                host: cfg.api.host.clone(),
                port: cfg.api.port,
                enable_debug_endpoints: cfg.api.enable_debug_endpoints,
                auth_mode: cfg.api.auth_mode,
                rate_limit_enabled: cfg.api.rate_limit_enabled,
                rate_limit_window_secs: cfg.api.rate_limit_window_secs,
                rate_limit_auth_bootstrap_max_per_window: cfg
                    .api
                    .rate_limit_auth_bootstrap_max_per_window,
                rate_limit_session_max_per_window: cfg.api.rate_limit_session_max_per_window,
                rate_limit_token_rotate_max_per_window: cfg
                    .api
                    .rate_limit_token_rotate_max_per_window,
            },
        }
    }
}

pub(crate) fn validate_settings(cfg: &Config) -> Result<(), StatusCode> {
    cfg.sam
        .host
        .parse::<std::net::IpAddr>()
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    if !(1..=65535).contains(&cfg.sam.port) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if cfg.sam.session_name.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    parse_api_bind_host(&cfg.api.host).map_err(|_| StatusCode::BAD_REQUEST)?;
    if !(1..=65535).contains(&cfg.api.port) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if cfg.api.rate_limit_window_secs == 0
        || cfg.api.rate_limit_auth_bootstrap_max_per_window == 0
        || cfg.api.rate_limit_session_max_per_window == 0
        || cfg.api.rate_limit_token_rotate_max_per_window == 0
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    EnvFilter::try_new(cfg.general.log_level.clone()).map_err(|_| StatusCode::BAD_REQUEST)?;
    EnvFilter::try_new(cfg.general.log_file_level.clone()).map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(())
}

pub(crate) fn apply_settings_patch(cfg: &mut Config, patch: SettingsPatchRequest) {
    if let Some(general) = patch.general {
        if let Some(log_level) = general.log_level {
            cfg.general.log_level = log_level.trim().to_string();
        }
        if let Some(log_to_file) = general.log_to_file {
            cfg.general.log_to_file = log_to_file;
        }
        if let Some(log_file_level) = general.log_file_level {
            cfg.general.log_file_level = log_file_level.trim().to_string();
        }
        if let Some(auto_open_ui) = general.auto_open_ui {
            cfg.general.auto_open_ui = auto_open_ui;
        }
    }

    if let Some(sam) = patch.sam {
        if let Some(host) = sam.host {
            cfg.sam.host = host.trim().to_string();
        }
        if let Some(port) = sam.port {
            cfg.sam.port = port;
        }
        if let Some(session_name) = sam.session_name {
            cfg.sam.session_name = session_name.trim().to_string();
        }
    }

    if let Some(api) = patch.api {
        if let Some(host) = api.host {
            cfg.api.host = host.trim().to_string();
        }
        if let Some(port) = api.port {
            cfg.api.port = port;
        }
        if let Some(enable_debug_endpoints) = api.enable_debug_endpoints {
            cfg.api.enable_debug_endpoints = enable_debug_endpoints;
        }
        if let Some(auth_mode) = api.auth_mode {
            cfg.api.auth_mode = auth_mode;
        }
        if let Some(rate_limit_enabled) = api.rate_limit_enabled {
            cfg.api.rate_limit_enabled = rate_limit_enabled;
        }
        if let Some(rate_limit_window_secs) = api.rate_limit_window_secs {
            cfg.api.rate_limit_window_secs = rate_limit_window_secs;
        }
        if let Some(rate_limit_auth_bootstrap_max_per_window) =
            api.rate_limit_auth_bootstrap_max_per_window
        {
            cfg.api.rate_limit_auth_bootstrap_max_per_window =
                rate_limit_auth_bootstrap_max_per_window;
        }
        if let Some(rate_limit_session_max_per_window) = api.rate_limit_session_max_per_window {
            cfg.api.rate_limit_session_max_per_window = rate_limit_session_max_per_window;
        }
        if let Some(rate_limit_token_rotate_max_per_window) =
            api.rate_limit_token_rotate_max_per_window
        {
            cfg.api.rate_limit_token_rotate_max_per_window = rate_limit_token_rotate_max_per_window;
        }
    }
}

pub(crate) async fn settings_get(
    State(state): State<ApiState>,
) -> Result<Json<SettingsResponse>, StatusCode> {
    let cfg = state.config.lock().await;
    Ok(Json(SettingsResponse {
        settings: SettingsPayload::from_config(&cfg),
        restart_required: true,
    }))
}

pub(crate) async fn settings_patch(
    State(state): State<ApiState>,
    Json(patch): Json<SettingsPatchRequest>,
) -> Result<Json<SettingsResponse>, StatusCode> {
    let mut cfg = state.config.lock().await;
    let mut next = cfg.clone();
    apply_settings_patch(&mut next, patch);
    validate_settings(&next)?;
    next.persist_to(state.config_path.as_path())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    *cfg = next.clone();

    Ok(Json(SettingsResponse {
        settings: SettingsPayload::from_config(&next),
        restart_required: true,
    }))
}
