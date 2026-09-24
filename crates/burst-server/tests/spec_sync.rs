//! Validates that Rust response structs stay in sync with the OpenAPI spec.
//!
//! These tests serialize example instances of each response type and compare
//! the resulting JSON field names against the properties declared in
//! `specs/burst-api.yaml`. Any field present in one but not the other fails
//! the test, catching drift between spec and code at compile-time.
//!
//! No database or network required — runs with `cargo test`.

use std::collections::{BTreeSet, HashMap};

use serde::Serialize;
use serde_json::Value;

// ── OpenAPI spec helpers ────────────────────────────────────────────────────

struct SpecSchema {
    properties: BTreeSet<String>,
    required: BTreeSet<String>,
}

fn load_spec_schemas() -> HashMap<String, SpecSchema> {
    let yaml = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../specs/burst-api.yaml"),
    )
    .expect("failed to read burst-api.yaml");

    let doc: Value = serde_yaml::from_str(&yaml).expect("failed to parse spec");

    let schemas = doc["components"]["schemas"]
        .as_object()
        .expect("no components/schemas in spec");

    schemas
        .iter()
        .map(|(name, schema)| {
            let properties = schema["properties"]
                .as_object()
                .map(|p| p.keys().cloned().collect())
                .unwrap_or_default();

            let required = schema["required"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();

            (
                name.clone(),
                SpecSchema {
                    properties,
                    required,
                },
            )
        })
        .collect()
}

/// Compare JSON field names from a serialized Rust struct against the spec schema.
fn assert_fields_match<T: Serialize>(type_name: &str, spec_name: &str, example: &T) {
    let schemas = load_spec_schemas();
    let spec = schemas
        .get(spec_name)
        .unwrap_or_else(|| panic!("schema '{spec_name}' not found in spec"));

    let json = serde_json::to_value(example).expect("failed to serialize");
    let rust_fields: BTreeSet<String> = json
        .as_object()
        .expect("expected JSON object")
        .keys()
        .cloned()
        .collect();

    // Fields in spec but missing from Rust
    let missing_from_rust: BTreeSet<_> = spec.required.difference(&rust_fields).collect();
    assert!(
        missing_from_rust.is_empty(),
        "{type_name}: required spec fields missing from Rust struct: {missing_from_rust:?}"
    );

    // Fields in Rust but not in spec at all
    let extra_in_rust: BTreeSet<_> = rust_fields.difference(&spec.properties).collect();
    assert!(
        extra_in_rust.is_empty(),
        "{type_name}: Rust struct has fields not in spec: {extra_in_rust:?}"
    );
}

// ── Example builders (all fields populated so they appear in JSON) ──────────

use burst_server::api::attachments::AttachmentResponse;
use burst_server::api::channels::{
    ChannelMemberResponse, ChannelResponse, MessageResponse, ReactionResponse,
};
use burst_server::api::search::SearchResultResponse;
use burst_server::api::users::{DoNotDisturbResponse, ScheduleBody, UserResponse};

fn example_user_response() -> UserResponse {
    UserResponse {
        id: "usr_00000000-0000-0000-0000-000000000001".into(),
        username: "alice".into(),
        display_name: "Alice".into(),
        email: Some("alice@example.com".into()),
        avatar_url: Some("https://example.com/avatar.png".into()),
        role: "member".into(),
        status: "online".into(),
        status_text: Some("Hello".into()),
        status_emoji: Some("👋".into()),
        status_expires_at: Some("2026-01-02T00:00:00+00:00".into()),
        do_not_disturb_until: Some("2026-01-02T00:00:00+00:00".into()),
        is_bot: false,
        created_at: "2026-01-01T00:00:00Z".into(),
    }
}

fn example_channel_response() -> ChannelResponse {
    ChannelResponse {
        id: "ch_00000000-0000-0000-0000-000000000010".into(),
        kind: "public".into(),
        name: Some("general".into()),
        slug: Some("general".into()),
        topic: Some("General chat".into()),
        description: Some("The general channel".into()),
        created_by: "usr_00000000-0000-0000-0000-000000000001".into(),
        is_archived: false,
        is_readonly: false,
        unread_count: 0,
        created_at: "2026-01-01T00:00:00Z".into(),
        updated_at: "2026-01-01T00:00:00Z".into(),
    }
}

fn example_channel_member_response() -> ChannelMemberResponse {
    ChannelMemberResponse {
        user_id: "usr_00000000-0000-0000-0000-000000000001".into(),
        role: "member".into(),
        joined_at: "2026-01-01T00:00:00Z".into(),
    }
}

fn example_message_response() -> MessageResponse {
    MessageResponse {
        id: "msg_00000000-0000-0000-0000-000000000100".into(),
        channel_id: "ch_00000000-0000-0000-0000-000000000010".into(),
        user_id: "usr_00000000-0000-0000-0000-000000000001".into(),
        thread_id: Some("msg_00000000-0000-0000-0000-000000000099".into()),
        content: "Hello, world!".into(),
        edited_at: Some("2026-01-01T01:00:00Z".into()),
        deleted_at: Some("2026-01-01T02:00:00Z".into()),
        reply_count: 0,
        reactions: vec![ReactionResponse {
            emoji: "👍".into(),
            count: 1,
            user_ids: vec!["usr_00000000-0000-0000-0000-000000000001".into()],
        }],
        attachments: vec![],
        created_at: "2026-01-01T00:00:00Z".into(),
    }
}

fn example_reaction_count() -> ReactionResponse {
    ReactionResponse {
        emoji: "👍".into(),
        count: 1,
        user_ids: vec!["usr_00000000-0000-0000-0000-000000000001".into()],
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[test]
fn do_not_disturb_matches_spec() {
    let schedule = ScheduleBody {
        start: "22:00".into(),
        end: "07:00".into(),
        days: vec!["mon".into()],
        time_zone: "Europe/Paris".into(),
    };
    assert_fields_match("ScheduleBody", "DoNotDisturbSchedule", &schedule);
    let response = DoNotDisturbResponse {
        snooze_until: Some("2026-01-02T00:00:00+00:00".into()),
        schedule: Some(schedule),
        quiet_until: Some("2026-01-02T00:00:00+00:00".into()),
    };
    assert_fields_match("DoNotDisturbResponse", "DoNotDisturb", &response);
}

#[test]
fn export_response_matches_spec() {
    let example = burst_server::api::admin::ExportResponse {
        id: "exp_00000000-0000-0000-0000-000000000001".into(),
        scope: "channel".into(),
        channel_id: Some("ch_00000000-0000-0000-0000-000000000010".into()),
        status: "completed".into(),
        requested_by: "usr_00000000-0000-0000-0000-000000000001".into(),
        size_bytes: Some(1024),
        error: Some("interrupted".into()),
        created_at: "2026-01-01T00:00:00Z".into(),
        completed_at: Some("2026-01-01T00:01:00Z".into()),
    };
    assert_fields_match("ExportResponse", "Export", &example);
}

#[test]
fn user_response_matches_spec() {
    assert_fields_match("UserResponse", "User", &example_user_response());
}

#[test]
fn channel_response_matches_spec() {
    assert_fields_match("ChannelResponse", "Channel", &example_channel_response());
}

#[test]
fn channel_member_response_matches_spec() {
    assert_fields_match(
        "ChannelMemberResponse",
        "ChannelMember",
        &example_channel_member_response(),
    );
}

#[test]
fn message_response_matches_spec() {
    assert_fields_match("MessageResponse", "Message", &example_message_response());
}

#[test]
fn reaction_count_matches_spec() {
    assert_fields_match(
        "ReactionResponse",
        "ReactionCount",
        &example_reaction_count(),
    );
}

fn example_attachment_response() -> AttachmentResponse {
    AttachmentResponse {
        id: "att_00000000-0000-0000-0000-000000000200".into(),
        file_name: "report.pdf".into(),
        file_size: 1024,
        content_type: "application/pdf".into(),
        metadata: serde_json::json!({}),
        created_at: "2026-01-01T00:00:00Z".into(),
    }
}

fn example_search_result_response() -> SearchResultResponse {
    SearchResultResponse {
        id: "msg_00000000-0000-0000-0000-000000000300".into(),
        channel_id: "ch_00000000-0000-0000-0000-000000000010".into(),
        user_id: "usr_00000000-0000-0000-0000-000000000001".into(),
        thread_id: Some("msg_00000000-0000-0000-0000-000000000099".into()),
        content: "matching content".into(),
        headline: "<mark>matching</mark> content".into(),
        created_at: "2026-01-01T00:00:00Z".into(),
        // Set, so the field is serialised and checked against the spec.
        matched_file: Some("q3-report.pdf".into()),
    }
}

#[test]
fn attachment_response_matches_spec() {
    assert_fields_match(
        "AttachmentResponse",
        "Attachment",
        &example_attachment_response(),
    );
}

#[test]
fn search_result_response_matches_spec() {
    assert_fields_match(
        "SearchResultResponse",
        "SearchResult",
        &example_search_result_response(),
    );
}
