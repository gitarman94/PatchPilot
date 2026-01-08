diesel::table! {
    devices (id) {
        id -> BigInt,
        device_id -> BigInt,
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
        device_type -> Text,
        device_model -> Text,
        uptime -> Nullable<BigInt>,
        updates_available -> Bool,
        network_interfaces -> Nullable<Text>,
        ip_address -> Nullable<Text>,
    }
}

diesel::table! {
    actions (id) {
        id -> BigInt,
        action_type -> Text,
        parameters -> Nullable<Text>,
        author -> Nullable<Text>,
        created_at -> Timestamp,
        expires_at -> Timestamp,
        canceled -> Bool,
    }
}

diesel::table! {
    action_targets (id) {
        id -> BigInt,
        action_id -> BigInt,
        device_id -> BigInt,
        status -> Text,
        last_update -> Timestamp,
        response -> Nullable<Text>,
    }
}

diesel::table! {
    history_log (id) {
        id -> BigInt,
        action_id -> BigInt,
        device_name -> Nullable<Text>,
        actor -> Nullable<Text>,
        action_type -> Text,
        details -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    audit (id) {
        id -> Integer,
        actor -> Text,
        action_type -> Text,
        target -> Nullable<Text>,
        details -> Nullable<Text>,
        created_at -> Timestamp,
    }
}

diesel::table! {
    users (id) {
        id -> Integer,
        username -> Text,
        password_hash -> Text,
        created_at -> Timestamp,
    }
}

diesel::table! {
    roles (id) {
        id -> Integer,
        name -> Text,
    }
}

diesel::table! {
    user_roles (id) {
        id -> Integer,
        user_id -> Integer,
        role_id -> Integer,
    }
}

diesel::table! {
    groups (id) {
        id -> Integer,
        name -> Text,
        description -> Nullable<Text>,
    }
}

diesel::table! {
    user_groups (id) {
        id -> Integer,
        user_id -> Integer,
        group_id -> Integer,
    }
}

diesel::table! {
    server_settings (id) {
        id -> Integer,
        auto_approve_devices -> Bool,
        auto_refresh_enabled -> Bool,
        auto_refresh_seconds -> BigInt,
        default_action_ttl_seconds -> BigInt,
        action_polling_enabled -> Bool,
        ping_target_ip -> Text,
        force_https -> Bool,
    }
}

diesel::joinable!(user_roles -> roles (role_id));
diesel::joinable!(user_roles -> users (user_id));
diesel::joinable!(user_groups -> users (user_id));
diesel::joinable!(user_groups -> groups (group_id));

diesel::allow_tables_to_appear_in_same_query!(
    devices,
    actions,
    action_targets,
    history_log,
    audit,
    users,
    roles,
    user_roles,
    groups,
    user_groups,
    server_settings,
);
