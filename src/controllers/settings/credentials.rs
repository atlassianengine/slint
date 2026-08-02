// Rust guideline compliant 2026-02-21

use std::rc::Rc;

use crate::{AppWindow, ConnectionItem};
use agent_auth::{
    credential_exists, delete_credential_secret, is_credential_ref, save_credential_secret,
};
use slint::VecModel;

use super::connection_items::refresh_ai_connection_key_hint;
use super::constants::{
    AiProviderCredentialBinding, AI_PROVIDER_CREDENTIALS, CLAUDE_API_KEY_FIELD_ID,
    GEMINI_API_KEY_FIELD_ID, OPENAI_API_KEY_FIELD_ID,
};

pub(crate) fn hydrate_ai_provider_credentials(
    app: &AppWindow,
    connections: &Rc<VecModel<ConnectionItem>>,
) {
    for binding in AI_PROVIDER_CREDENTIALS {
        if let Some(stored_ref) = find_stored_ai_provider_ref(binding) {
            set_ai_provider_field_value(app, binding, &stored_ref);
            refresh_ai_connection_key_hint(connections, binding.connection_id, true);
        } else {
            set_ai_provider_field_value(app, binding, "");
            refresh_ai_connection_key_hint(connections, binding.connection_id, false);
        }
    }
}

pub(crate) fn sync_ai_provider_credential(
    app: &AppWindow,
    connections: &Rc<VecModel<ConnectionItem>>,
    binding: AiProviderCredentialBinding,
) -> Result<Option<&'static str>, String> {
    let requested_key = read_ai_provider_field_value(app, binding)
        .trim()
        .to_string();
    if requested_key.is_empty() {
        let current_value = read_ai_provider_field_value(app, binding)
            .trim()
            .to_string();
        if is_credential_ref(&current_value) {
            let _ = delete_credential_secret(&current_value);
        }
        set_ai_provider_field_value(app, binding, "");
        refresh_ai_connection_key_hint(connections, binding.connection_id, false);
        return Ok(None);
    }

    let resolved = if is_credential_ref(&requested_key) {
        if !credential_exists(&requested_key)? {
            return Err(format!(
                "No credential found in secure store for '{}'.",
                binding.label
            ));
        }
        requested_key
    } else {
        save_credential_secret(binding.credential_ref, &requested_key)?;
        if !credential_exists(binding.credential_ref)? {
            return Err(format!(
                "Could not verify {} API key write to OS keychain.",
                binding.label
            ));
        }
        binding.credential_ref.to_string()
    };

    set_ai_provider_field_value(app, binding, &resolved);
    refresh_ai_connection_key_hint(connections, binding.connection_id, true);
    Ok(Some(binding.label))
}

fn find_stored_ai_provider_ref(binding: AiProviderCredentialBinding) -> Option<String> {
    credential_exists(binding.credential_ref)
        .ok()
        .filter(|exists| *exists)
        .map(|_| binding.credential_ref.to_string())
}

fn read_ai_provider_field_value(app: &AppWindow, binding: AiProviderCredentialBinding) -> String {
    match binding.field_id {
        GEMINI_API_KEY_FIELD_ID => app.get_settings_gemini_api_key().to_string(),
        CLAUDE_API_KEY_FIELD_ID => app.get_settings_claude_api_key().to_string(),
        OPENAI_API_KEY_FIELD_ID => app.get_settings_openai_api_key().to_string(),
        _ => String::new(),
    }
}

fn set_ai_provider_field_value(app: &AppWindow, binding: AiProviderCredentialBinding, value: &str) {
    match binding.field_id {
        GEMINI_API_KEY_FIELD_ID => app.set_settings_gemini_api_key(value.into()),
        CLAUDE_API_KEY_FIELD_ID => app.set_settings_claude_api_key(value.into()),
        OPENAI_API_KEY_FIELD_ID => app.set_settings_openai_api_key(value.into()),
        _ => {}
    }
}
