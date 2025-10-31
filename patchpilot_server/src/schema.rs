// @generated automatically by Diesel CLI.

diesel::table! {
    devices (id) {
        id -> Integer,
        device_name -> Text,
        hostname -> Text,
        os_name -> Text,
        architecture -> Text,
        last_checkin -> Timestamp,
        approved -> Bool,
        cpu -> Float,
        ram_total -> BigInt,
        ram_used -> BigInt,
        ram_free -> BigInt,
        disk_total -> BigInt,
        disk_free -> BigInt,
        disk_health -> Text,
        network_throughput -> BigInt,
        ping_latency -> Float,
        device_type -> Text,
        device_model -> Text,
    }
}
