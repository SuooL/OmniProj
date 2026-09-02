use std::path::PathBuf;

use keyring::Entry;
use serde::{Deserialize, Serialize};
use toml_edit::{value, DocumentMut, Item, Table};

use crate::error::{CommandError, CommandResult, ErrorCode};

const KEYCHAIN_SERVICE: &str = "app.omniproj.desktop.llm";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentProviderDto {
    pub name: String,
    pub kind: String,
    pub local: bool,
    pub key_required: bool,
    pub key_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentSettingsDto {
    pub default_model: String,
    pub selected_provider: String,
    pub selected_model: String,
    pub remote_consent: bool,
    pub ready: bool,
    pub providers: Vec<AgentProviderDto>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SaveAgentSettingsInput {
    pub default_model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    pub remote_consent: bool,
}

fn provider_and_model(model: &str) -> CommandResult<(&str, &str)> {
    let (provider, model) = model.trim().split_once('/').ok_or_else(|| {
        CommandError::invalid_input("model must use provider/model format")
            .with_field("default_model")
    })?;
    if provider.is_empty() || model.is_empty() {
        return Err(
            CommandError::invalid_input("provider and model must both be non-empty")
                .with_field("default_model"),
        );
    }
    Ok((provider, model))
}

fn keychain_entry(provider: &str) -> CommandResult<Entry> {
    Entry::new(KEYCHAIN_SERVICE, provider).map_err(|error| {
        CommandError::new(
            ErrorCode::StoreWriteFailed,
            format!("system credential store is unavailable: {error}"),
        )
    })
}

fn keychain_key(provider: &str) -> Option<String> {
    keychain_entry(provider)
        .ok()
        .and_then(|entry| entry.get_password().ok())
        .filter(|key| !key.is_empty())
}

fn environment_key(provider: &str) -> Option<String> {
    omniproj_distill::list()
        .into_iter()
        .find(|status| status.name == provider)
        .and_then(|status| status.api_key_env)
        .and_then(|name| std::env::var(name).ok())
        .filter(|key| !key.is_empty())
}

fn effective_key(provider: &str) -> Option<String> {
    environment_key(provider).or_else(|| keychain_key(provider))
}

pub fn get_agent_settings() -> CommandResult<AgentSettingsDto> {
    let default_model = omniproj_distill::default_model_string();
    let (selected_provider, selected_model) = provider_and_model(&default_model)?;
    let selected_provider = selected_provider.to_owned();
    let selected_model = selected_model.to_owned();
    let config = omniproj_distill::config::load();
    let remote_consent = config.privacy.consented();
    let selected_key = effective_key(&selected_provider);
    let providers = omniproj_distill::list()
        .into_iter()
        .map(|status| {
            let local = omniproj_distill::is_local_provider(&status.name);
            let key_required = status.api_key_env.is_some() && !local;
            let key_present = if status.name == selected_provider {
                !key_required || selected_key.is_some()
            } else {
                !key_required || status.key_present
            };
            AgentProviderDto {
                name: status.name,
                kind: status.kind.as_str().to_owned(),
                local,
                key_required,
                key_present,
            }
        })
        .collect::<Vec<_>>();
    let selected = providers
        .iter()
        .find(|provider| provider.name == selected_provider)
        .ok_or_else(|| CommandError::invalid_input("selected provider is not configured"))?;
    let ready = selected.key_present && (selected.local || remote_consent);
    Ok(AgentSettingsDto {
        default_model,
        selected_provider,
        selected_model,
        remote_consent,
        ready,
        providers,
    })
}

fn write_config(default_model: &str, remote_consent: bool) -> CommandResult<()> {
    let path = omniproj_distill::config_path();
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut document = if text.trim().is_empty() {
        DocumentMut::new()
    } else {
        text.parse::<DocumentMut>().map_err(|error| {
            CommandError::new(
                ErrorCode::StoreReadFailed,
                format!("cannot edit ~/.omniproj/config.toml: {error}"),
            )
        })?
    };
    document["default_model"] = value(default_model);
    if !document["privacy"].is_table() {
        document["privacy"] = Item::Table(Table::new());
    }
    document["privacy"]["send_consent"] = value(remote_consent);
    omniproj_core::ensure_home_then_write(|| -> CommandResult<()> {
        omniproj_core::atomic_write(&path, document.to_string().as_bytes())?;
        omniproj_core::commit_paths_checked(
            "settings: update agent provider",
            &[PathBuf::from("config.toml")],
        )?;
        Ok(())
    })
}

pub fn save_agent_settings(input: SaveAgentSettingsInput) -> CommandResult<AgentSettingsDto> {
    let (provider, _) = provider_and_model(&input.default_model)?;
    let provider_exists = omniproj_distill::list()
        .iter()
        .any(|status| status.name == provider);
    if !provider_exists {
        return Err(
            CommandError::invalid_input("provider is not configured").with_field("default_model")
        );
    }
    let local = omniproj_distill::is_local_provider(provider);
    if !local && !input.remote_consent {
        return Err(CommandError::invalid_input(
            "confirm remote transmission before selecting a remote provider",
        )
        .with_field("remote_consent"));
    }
    if let Some(api_key) = input
        .api_key
        .as_deref()
        .map(str::trim)
        .filter(|key| !key.is_empty())
    {
        keychain_entry(provider)?
            .set_password(api_key)
            .map_err(|error| {
                CommandError::new(
                    ErrorCode::StoreWriteFailed,
                    format!("could not save API key to the system credential store: {error}"),
                )
            })?;
    }
    write_config(input.default_model.trim(), input.remote_consent)?;
    get_agent_settings()
}

pub fn resolve_provider() -> CommandResult<omniproj_distill::config::Resolved> {
    let settings = get_agent_settings()?;
    let local = omniproj_distill::is_local_provider(&settings.default_model);
    if !local && !settings.remote_consent {
        return Err(CommandError::invalid_input(
            "remote Agent use requires explicit transmission consent in Agent settings",
        ));
    }
    let key = effective_key(&settings.selected_provider);
    omniproj_distill::resolve_with_api_key(Some(&settings.default_model), key.as_deref()).map_err(
        |error| {
            CommandError::new(
                ErrorCode::InvalidInput,
                format!("Agent provider is not ready: {error}"),
            )
        },
    )
}

pub async fn test_agent_provider() -> CommandResult<()> {
    use omniproj_distill::LlmProvider;
    let resolved = resolve_provider()?;
    resolved
        .provider
        .complete(
            "Reply with exactly OK. This is a provider connectivity test.",
            "OK",
        )
        .await
        .map(|_| ())
        .map_err(|error| {
            CommandError::new(
                ErrorCode::SourceObservationFailed,
                format!("Agent provider connection failed: {error}"),
            )
            .retryable()
        })
}
