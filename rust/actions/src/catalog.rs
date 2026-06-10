use crate::{ActionMetadata, ActionRisk, ActionSideEffect};

pub const ANDROID_ACTION_NAMES: &[&str] = &[
    "device_info",
    "system_info",
    "network_status",
    "power_status",
    "set_volume",
    "set_silent_mode",
    "storage_info",
    "get_location",
    "foreground_app",
    "list_installed_apps",
    "launch_app",
    "set_alarm",
    "set_timer",
    "list_alarms",
    "read_sms",
    "send_sms",
    "read_call_log",
    "place_call",
    "search_contacts",
    "list_notifications",
    "clipboard_copy",
    "clipboard_read",
    "wifi_toggle",
    "bluetooth_toggle",
    "media_play_pause",
    "media_now_playing",
    "screenshot",
    "screen_record",
    "take_photo",
    "record_video",
    "record_audio",
    "open_webpage",
    "select_file",
    "search_files",
    "list_calendar_events",
    "create_calendar_event",
    "check_permissions",
    "read_file",
    "http_call",
    "intent_set_alarm",
    "intent_set_timer",
    "intent_show_alarms",
    "intent_insert_calendar",
    "intent_capture_return",
    "intent_camera_still",
    "intent_camera_video",
    "intent_pick_contact",
    "intent_pick_contact_data",
    "intent_view_contact",
    "intent_edit_contact",
    "intent_insert_contact",
    "intent_compose_email",
    "intent_get_content",
    "intent_open_document",
    "intent_call_car",
    "intent_show_map",
    "intent_play_media",
    "intent_play_search",
    "intent_create_note",
];

pub const TAURI_LOCAL_ACTION_NAMES: &[&str] = &["echo", "text", "uppercase"];

pub fn metadata_for_action(name: &str) -> Option<ActionMetadata> {
    let metadata = match name {
        "device_info" | "system_info" | "network_status" | "power_status" | "storage_info"
        | "list_installed_apps" | "check_permissions" | "media_now_playing" => {
            read_only(ActionRisk::Low, 10_000, false)
        }
        "get_location" | "foreground_app" | "read_sms" | "read_call_log"
        | "search_contacts" | "list_notifications" | "clipboard_read"
        | "list_calendar_events" | "read_file" => {
            read_only(ActionRisk::High, 15_000, true)
        }
        "search_files" => read_only(ActionRisk::Medium, 15_000, false),
        "http_call" | "open_webpage" => idempotent(ActionRisk::High, 20_000, true),
        "set_volume" | "set_silent_mode" | "wifi_toggle" | "bluetooth_toggle"
        | "media_play_pause" | "clipboard_copy" => {
            idempotent(ActionRisk::Medium, 10_000, false)
        }
        "launch_app" | "set_alarm" | "set_timer" | "list_alarms" | "select_file" => {
            interactive(ActionRisk::Medium, 30_000)
        }
        "send_sms" | "place_call" => interactive(ActionRisk::Critical, 30_000),
        "create_calendar_event" => interactive(ActionRisk::High, 30_000),
        "screenshot" | "screen_record" | "take_photo" | "record_video" | "record_audio" => {
            evidence_capture(60_000)
        }
        "intent_set_alarm"
        | "intent_set_timer"
        | "intent_show_alarms"
        | "intent_insert_calendar"
        | "intent_capture_return"
        | "intent_camera_still"
        | "intent_camera_video"
        | "intent_pick_contact"
        | "intent_pick_contact_data"
        | "intent_view_contact"
        | "intent_edit_contact"
        | "intent_insert_contact"
        | "intent_compose_email"
        | "intent_get_content"
        | "intent_open_document"
        | "intent_call_car"
        | "intent_show_map"
        | "intent_play_media"
        | "intent_play_search"
        | "intent_create_note" => interactive(ActionRisk::High, 60_000),
        "echo" | "text" | "uppercase" => read_only(ActionRisk::Low, 5_000, false),
        _ => return None,
    };
    Some(metadata)
}

fn read_only(risk: ActionRisk, timeout_ms: u64, sensitive: bool) -> ActionMetadata {
    ActionMetadata {
        side_effect: ActionSideEffect::Pure,
        risk,
        requires_confirmation: sensitive,
        collect_evidence: sensitive,
        timeout_ms,
        max_retries: 1,
        callable_by_subagent: true,
    }
}

fn idempotent(risk: ActionRisk, timeout_ms: u64, sensitive: bool) -> ActionMetadata {
    ActionMetadata {
        side_effect: ActionSideEffect::Idempotent,
        risk,
        requires_confirmation: sensitive,
        collect_evidence: sensitive,
        timeout_ms,
        max_retries: 1,
        callable_by_subagent: true,
    }
}

fn interactive(risk: ActionRisk, timeout_ms: u64) -> ActionMetadata {
    ActionMetadata {
        side_effect: ActionSideEffect::NonIdempotent,
        risk,
        requires_confirmation: true,
        collect_evidence: true,
        timeout_ms,
        max_retries: 0,
        callable_by_subagent: true,
    }
}

fn evidence_capture(timeout_ms: u64) -> ActionMetadata {
    ActionMetadata {
        side_effect: ActionSideEffect::NonIdempotent,
        risk: ActionRisk::High,
        requires_confirmation: true,
        collect_evidence: true,
        timeout_ms,
        max_retries: 0,
        callable_by_subagent: true,
    }
}

#[cfg(test)]
mod tests {
    use super::{ANDROID_ACTION_NAMES, TAURI_LOCAL_ACTION_NAMES, metadata_for_action};
    use std::collections::HashSet;

    const BUILTIN_REGISTRAR: &str = include_str!(
        "../../../kotlin/kotlin-actions-runtime/src/main/kotlin/actions/BuiltinActionRegistrar.kt"
    );
    const INTENT_REGISTRAR: &str = include_str!(
        "../../../kotlin/kotlin-actions-runtime/src/main/kotlin/actions/IntentActionRegistrar.kt"
    );

    #[test]
    fn catalog_has_metadata_for_every_declared_action() {
        let names: Vec<&str> = ANDROID_ACTION_NAMES
            .iter()
            .chain(TAURI_LOCAL_ACTION_NAMES.iter())
            .copied()
            .collect();
        let unique: HashSet<&str> = names.iter().copied().collect();

        assert_eq!(unique.len(), names.len());
        assert_eq!(ANDROID_ACTION_NAMES.len(), 59);
        assert_eq!(TAURI_LOCAL_ACTION_NAMES.len(), 3);
        for name in names {
            assert!(
                metadata_for_action(name).is_some(),
                "missing metadata for {name}"
            );
        }
    }

    #[test]
    fn unknown_actions_have_no_metadata() {
        assert!(metadata_for_action("unknown_action").is_none());
    }

    #[test]
    fn android_catalog_matches_kotlin_registrars() {
        let registered: HashSet<String> = registered_names(BUILTIN_REGISTRAR)
            .into_iter()
            .chain(registered_names(INTENT_REGISTRAR))
            .collect();
        let catalog: HashSet<String> = ANDROID_ACTION_NAMES
            .iter()
            .map(|name| (*name).to_string())
            .collect();

        assert_eq!(catalog, registered);
    }

    fn registered_names(source: &str) -> Vec<String> {
        source
            .split("register(")
            .skip(1)
            .filter_map(|registration| {
                let start = registration.find('"')? + 1;
                let end = start + registration[start..].find('"')?;
                Some(registration[start..end].to_string())
            })
            .collect()
    }
}
