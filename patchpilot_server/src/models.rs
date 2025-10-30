use diesel::{Insertable, Queryable};
use chrono::{NaiveDateTime};
use serde::{Serialize, Deserialize};

#[derive(Queryable, Serialize, Deserialize)]
pub struct Device {
    pub id: i32,
    pub device_name: String,
    pub hostname: String,
    pub os_name: String,
    pub architecture: String,
    pub last_checkin: NaiveDateTime,
    pub approved: bool,
    pub cpu: f32,
    pub ram_total: i64,
    pub ram_used: i64,
    pub ram_free: i64,
    pub disk_total: i64,
    pub disk_free: i64,
    pub network_throughput: i64,
    pub ping_latency: Option<f32>,
    pub device_type: String,
    pub device_model: String,
}

#[derive(Insertable)]
#[table_name = "devices"]
pub struct NewDevice<'a> {
    pub device_name: &'a str,
    pub hostname: &'a str,
    pub os_name: &'a str,
    pub architecture: &'a str,
    pub last_checkin: NaiveDateTime,
    pub approved: bool,
    pub device_type: &'
