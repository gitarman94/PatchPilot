diesel::table! {
    devices (id) {
        id -> Integer,
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