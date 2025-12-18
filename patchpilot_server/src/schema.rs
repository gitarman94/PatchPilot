diesel::table! {
    devices (id) {
        id -> Integer,
        device_id -> Text,
        device_name -> Text,
        hostname -> Text,
        os_name -> Text,
        architecture -> Text,
        last_checkin -> Timestamp,
        approved -> Bool,

        cpu_usage -> Float,
        cpu_count -> Integer,
        cpu_brand -> Text,

        ram_total -> BigInt,
        ram_used -> BigInt,

        disk_total -> BigInt,
        disk_free -> BigInt,
        disk_health -> Text,

        network_throughput -> BigInt,
        ping_latency -> Nullable<Float>,

        device_type -> Text,
        device_model -> Text,
        uptime -> Nullable<Text>,
        updates_available -> Bool,

        network_interfaces -> Nullable<Text>,
        ip_address -> Nullable<Text>,
    }
}

diesel::table! {
    actions (id) {
        id -> Text,                 // device_id
        action_type -> Text,        // command | reboot | shutdown | force_update
        parameters -> Nullable<Text>,
        author -> Nullable<Text>,
        created_at -> Timestamp,
        expires_at -> Timestamp,
        canceled -> Bool,
    }
}

diesel::table! {
    action_targets (id) {
        id -> Integer,
        action_id -> Text,          // FK to actions.id
        device_id -> Text,        // device_id - store as text for simplicity
        status -> Text,             // pending | running | completed | failed | expired | canceled
        last_update -> Timestamp,
        response -> Nullable<Text>, // stdout/stderr or structured JSON
    }
}

diesel::table! {
    history_log (id) {
        id -> Integer,
        action_id -> Nullable<Text>,
        device_name -> Nullable<Text>,
        actor -> Nullable<Text>,
        action_type -> Text,
        details -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    audit_log (id) {
        id -> Integer,
        actor -> Text,               // who did it
        action_type -> Text,         // what they did (e.g., set_auto_refresh)
        target -> Nullable<Text>,    // optional: setting name, device_id, etc.
        details -> Nullable<Text>,   // optional: extra info
        created_at -> Timestamp,
    }
}

diesel::allow_tables_to_appear_in_same_query!(
    devices,
    actions,
    action_targets,
    history_log,
    audit_log,
);
