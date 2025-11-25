01 use diesel::prelude::*;
02 use diesel::r2d2::{ConnectionManager, PooledConnection};
03 use r2d2::Pool;
04 use rocket::{get, post, routes, launch, State};
05 use rocket::serde::json::Json;
06 use rocket::fs::{FileServer, NamedFile};
07 use flexi_logger::{Logger, FileSpec, Age, Cleanup, Criterion, Naming};
08 use log::info;
09 use serde_json::json;
10 use chrono::Utc;
11 use local_ip_address::local_ip;
12 use std::sync::Mutex;
13 
14 use sysinfo::System;
15 
16 mod schema;
17 mod models;
18 
19 use models::{Device, NewDevice, DeviceInfo};
20 use diesel::sqlite::SqliteConnection;
21 
22 type DbPool = Pool<ConnectionManager<SqliteConnection>>;
23 
24 pub struct AppState {
25     pub system: Mutex<System>,
26 }
27 
28 fn init_logger() {
29     Logger::try_with_str("info")
30         .unwrap()
31         .log_to_file(FileSpec::default().directory("logs"))
32         .rotate(
33             Criterion::Age(Age::Day),
34             Naming::Numbers,
35             Cleanup::KeepLogFiles(7),
36         )
37         .start()
38         .unwrap();
39 }
40 
41 #[derive(Debug)]
42 pub enum ApiError {
43     DbError(diesel::result::Error),
44     ValidationError(String),
45 }
46 
47 impl From<diesel::result::Error> for ApiError {
48     fn from(e: diesel::result::Error) -> Self {
49         ApiError::DbError(e)
50     }
51 }
52 
53 impl ApiError {
54     fn message(&self) -> String {
55         match self {
56             ApiError::DbError(e) => format!("Database error: {}", e),
57             ApiError::ValidationError(msg) => msg.clone(),
58         }
59     }
60 }
61 
62 fn establish_connection(pool: &DbPool)
63     -> Result<PooledConnection<ConnectionManager<SqliteConnection>>, ApiError> {
64     pool.get()
65         .map_err(|e| ApiError::ValidationError(format!("Failed to get DB connection: {}", e)))
66 }
67 
68 fn validate_device_info(info: &DeviceInfo) -> Result<(), ApiError> {
69     if info.system_info.cpu < 0.0 {
70         return Err(ApiError::ValidationError("CPU usage cannot be negative".into()));
71     }
72     if info.system_info.ram_total <= 0 {
73         return Err(ApiError::ValidationError("RAM total must be positive".into()));
74     }
75     Ok(())
76 }
77 
78 fn insert_or_update_device(
79     conn: &mut SqliteConnection,
80     device_id: &str,
81     info: &DeviceInfo
82 ) -> Result<Device, ApiError> {
83     use crate::schema::devices::dsl::*;
84 
85     let new_device = NewDevice::from_device_info(device_id, info);
86 
87     diesel::insert_into(devices)
88         .values(&new_device)
89         .on_conflict(device_name)
90         .do_update()
91         .set(&new_device)
92         .execute(conn)?;
93 
94     let updated_device = devices
95         .filter(device_name.eq(device_id))
96         .first::<Device>(conn)?;
97 
98     Ok(updated_device.enrich_for_dashboard())
99 }
100
101 #[post("/devices/<device_id>", format = "json", data = "<device_info>")]
102 async fn register_or_update_device(
103     pool: &State<DbPool>,
104     device_id: &str,
105     device_info: Json<DeviceInfo>,
106 ) -> Result<Json<Device>, String> {
107     validate_device_info(&device_info).map_err(|e| e.message())?;
108 
109     let pool = pool.inner().clone();
110     let device_info = device_info.into_inner();
111     let device_id = device_id.to_string();
112 
113     rocket::tokio::task::spawn_blocking(move || {
114         let mut conn = establish_connection(&pool).map_err(|e| e.message())?;
115         insert_or_update_device(&mut conn, &device_id, &device_info)
116             .map(Json)
117             .map_err(|e| e.message())
118     })
119     .await
120     .unwrap_or_else(|e| Err(format!("Task join error: {}", e)))
121 }
122
123 #[post("/devices/heartbeat", format = "json", data = "<payload>")]
124 async fn heartbeat(
125     pool: &State<DbPool>,
126     payload: Json<serde_json::Value>,
127 ) -> Json<serde_json::Value> {
128     use crate::schema::devices::dsl::*;
129 
130     let pool = pool.inner().clone();
131     let payload = payload.into_inner();
132 
133     rocket::tokio::task::spawn_blocking(move || {
134         if let Ok(mut conn) = pool.get() {
135             let device_id =
136                 payload.get("device_id").and_then(|v| v.as_str()).unwrap_or("unknown");
137             let device_type_val =
138                 payload.get("device_type").and_then(|v| v.as_str()).unwrap_or("unknown");
139             let device_model_val =
140                 payload.get("device_model").and_then(|v| v.as_str()).unwrap_or("unknown");
141             let network_interfaces_val =
142                 payload.get("network_interfaces").and_then(|v| v.as_str()).unwrap_or("");
143             let ip_address_val =
144                 payload.get("ip_address").and_then(|v| v.as_str()).unwrap_or("");
145 
146             let _ = diesel::insert_into(devices)
147                 .values((
148                     device_name.eq(device_id),
149                     hostname.eq(device_id),
150                     os_name.eq("unknown"),
151                     architecture.eq("unknown"),
152                     device_type.eq(device_type_val),
153                     device_model.eq(device_model_val),
154                     network_interfaces.eq(network_interfaces_val),
155                     ip_address.eq(ip_address_val),
156                     approved.eq(false),
157                     last_checkin.eq(Utc::now().naive_utc()),
158                     cpu.eq(0.0),
159                     ram_total.eq(0),
160                     ram_used.eq(0),
161                     ram_free.eq(0),
162                     disk_total.eq(0),
163                     disk_free.eq(0),
164                     disk_health.eq("unknown"),
165                     network_throughput.eq(0),
166                     ping_latency.eq(None::<f32>),
167                     uptime.eq(Some("0h 0m")),
168                     updates_available.eq(false)
169                 ))
170                 .on_conflict(device_name)
171                 .do_update()
172                 .set((
173                     last_checkin.eq(Utc::now().naive_utc()),
174                     network_interfaces.eq(network_interfaces_val),
175                     ip_address.eq(ip_address_val),
176                 ))
177                 .execute(&mut conn);
178         }
179     })
180     .await
181     .ok();
182 
183     Json(json!({"adopted": true}))
184 }
185
186 #[get("/devices")]
187 async fn get_devices(pool: &State<DbPool>) -> Result<Json<Vec<Device>>, String> {
188     let pool = pool.inner().clone();
189 
190     rocket::tokio::task::spawn_blocking(move || {
191         let mut conn = establish_connection(&pool).map_err(|e| e.message())?;
192         let results = crate::schema::devices::dsl::devices
193             .load::<Device>(&mut conn)
194             .map_err(|e| e.to_string())?
195             .into_iter()
196             .map(|d| d.enrich_for_dashboard())
197             .collect::<Vec<_>>();
198         Ok(Json(results))
199     })
200     .await
201     .unwrap_or_else(|e| Err(format!("Task join error: {}", e)))
202 }
203
204 #[get("/status")]
205 fn status(state: &State<AppState>) -> Json<serde_json::Value> {
206     let mut sys = state.system.lock().unwrap();
207     sys.refresh_all();
208 
209     Json(json!({
210         "server_time": Utc::now().to_rfc3339(),
211         "status": "ok",
212         "uptime_seconds": sysinfo::System::uptime(),
213         "cpu_count": sys.cpus().len(),
214         "cpu_usage_per_core_percent": sys.cpus().iter().map(|c| c.cpu_usage()).collect::<Vec<f32>>(),
215         "total_memory_bytes": sys.total_memory(),
216         "used_memory_bytes": sys.used_memory(),
217         "memory_usage_percent": if sys.total_memory() > 0 {
218             (sys.used_memory() as f32 / sys.total_memory() as f32) * 100.0
219         } else { 0.0 },
220         "total_swap_bytes": sys.total_swap(),
221         "used_swap_bytes": sys.used_swap(),
222         "swap_usage_percent": if sys.total_swap() > 0 {
223             (sys.used_swap() as f32 / sys.total_swap() as f32) * 100.0
224         } else { 0.0 },
225     }))
226 }
227
228 #[get("/")]
229 async fn dashboard() -> Option<NamedFile> {
230     NamedFile::open("/opt/patchpilot_server/templates/dashboard.html")
231         .await
232         .ok()
233 }
234
235 #[get("/favicon.ico")]
236 async fn favicon() -> Option<NamedFile> {
237     NamedFile::open("/opt/patchpilot_server/static/favicon.ico")
238         .await
239         .ok()
240 }
241
242 fn initialize_db(conn: &mut SqliteConnection) -> Result<(), diesel::result::Error> {
243     diesel::sql_query(r#"
244         CREATE TABLE IF NOT EXISTS devices (
245             id INTEGER PRIMARY KEY AUTOINCREMENT,
246             device_name TEXT NOT NULL UNIQUE,
247             hostname TEXT,
248             os_name TEXT,
249             architecture TEXT,
250             last_checkin TIMESTAMP NOT NULL,
251             approved BOOLEAN NOT NULL,
252             cpu FLOAT NOT NULL DEFAULT 0.0,
253             ram_total BIGINT NOT NULL DEFAULT 0,
254             ram_used BIGINT NOT NULL DEFAULT 0,
255             ram_free BIGINT NOT NULL DEFAULT 0,
256             disk_total BIGINT NOT NULL DEFAULT 0,
257             disk_free BIGINT NOT NULL DEFAULT 0,
258             disk_health TEXT,
259             network_throughput BIGINT NOT NULL DEFAULT 0,
260             ping_latency FLOAT,
261             device_type TEXT NOT NULL,
262             device_model TEXT NOT NULL,
263             uptime TEXT,
264             updates_available BOOLEAN NOT NULL DEFAULT 0,
265             network_interfaces TEXT,
266             ip_address TEXT
267         )
268     "#).execute(conn)?;
269     Ok(())
270 }
271
272 fn get_server_ip() -> String {
273     local_ip().map(|ip| ip.to_string()).unwrap_or_else(|_| "127.0.0.1".into())
274 }
275
276 #[launch]
277 fn rocket() -> _ {
278     init_logger();
279 
280     let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
281     let manager = ConnectionManager::<SqliteConnection>::new(database_url);
282     let pool = Pool::builder().build(manager).expect("Failed to create DB pool");
283 
284     {
285         let mut conn = pool.get().expect("DB connect failed");
286         initialize_db(&mut conn).expect("DB init failed");
287         info!("Database ready");
288     }
289 
290     let ip = get_server_ip();
291     let port = 8080;
292     info!("Server running at http://{}:{}/", ip, port);
293 
294     rocket::build()
295         .manage(pool)
296         .manage(AppState {
297             system: Mutex::new(System::new_all()),
298         })
299         .mount(
300             "/api",
301             routes![
302                 register_or_update_device,
303                 get_devices,
304                 status,
305                 heartbeat
306             ],
307         )
308         .mount("/", routes![dashboard, favicon])
309         .mount("/static", FileServer::from("/opt/patchpilot_server/static"))
310 }
