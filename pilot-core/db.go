package main

import (
	"database/sql"
	"fmt"
	"log"
	"strings"

	_ "github.com/mattn/go-sqlite3"
)

func initDB(db *sql.DB) {
	mustExec(db, `
CREATE TABLE IF NOT EXISTS devices (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	agent_id TEXT,
	hostname TEXT,
	fqdn TEXT,
	ip TEXT,
	os TEXT,
	architecture TEXT,
	device_type TEXT,
	device_model TEXT,
	cpu_model TEXT,
	cpu_usage REAL DEFAULT 0,
	ram_total INTEGER DEFAULT 0,
	ram_used INTEGER DEFAULT 0,
	ram_usage_percent REAL DEFAULT 0,
	disk_total INTEGER DEFAULT 0,
	disk_used INTEGER DEFAULT 0,
	disk_free INTEGER DEFAULT 0,
	disk_free_human TEXT,
	last_seen TEXT,
	approved INTEGER DEFAULT 0
);
`)

	mustExec(db, `CREATE UNIQUE INDEX IF NOT EXISTS idx_devices_agent_id ON devices(agent_id);`)

	mustExec(db, `
CREATE TABLE IF NOT EXISTS actions (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	name TEXT,
	device_id INTEGER,
	status TEXT,
	created_at TEXT,
	updated_at TEXT
);
`)

	mustExec(db, `
CREATE TABLE IF NOT EXISTS history (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	action TEXT,
	device_id INTEGER DEFAULT 0,
	created_at TEXT
);
`)

	mustExec(db, `
CREATE TABLE IF NOT EXISTS users (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	username TEXT UNIQUE,
	password_hash TEXT,
	role_id INTEGER
);
`)

	mustExec(db, `
CREATE TABLE IF NOT EXISTS roles (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	name TEXT UNIQUE
);
`)

	mustExec(db, `
CREATE TABLE IF NOT EXISTS groups (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	name TEXT UNIQUE
);
`)

	mustExec(db, `
CREATE TABLE IF NOT EXISTS user_groups (
	user_id INTEGER,
	group_id INTEGER
);
`)

	mustExec(db, `
CREATE TABLE IF NOT EXISTS settings (
	key TEXT PRIMARY KEY,
	value TEXT
);
`)

	mustExec(db, `
CREATE TABLE IF NOT EXISTS sessions (
	token TEXT PRIMARY KEY,
	username TEXT NOT NULL,
	created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
`)

	mustExec(db, `
CREATE TABLE IF NOT EXISTS schema_migrations (
	version INTEGER PRIMARY KEY
);
`)

	mustExec(db, `
CREATE TABLE IF NOT EXISTS agent_updates (
	id INTEGER PRIMARY KEY AUTOINCREMENT,
	version TEXT NOT NULL,
	platform TEXT NOT NULL,
	arch TEXT NOT NULL,
	filename TEXT NOT NULL,
	original_name TEXT,
	sha256 TEXT NOT NULL,
	size_bytes INTEGER DEFAULT 0,
	active INTEGER DEFAULT 0,
	uploaded_at TEXT DEFAULT CURRENT_TIMESTAMP
);
`)

	ensureColumn(db, "devices", "agent_id", "TEXT")
	ensureColumn(db, "devices", "fqdn", "TEXT")
	ensureColumn(db, "devices", "architecture", "TEXT")
	ensureColumn(db, "devices", "device_type", "TEXT")
	ensureColumn(db, "devices", "device_model", "TEXT")
	ensureColumn(db, "devices", "cpu_model", "TEXT")
	ensureColumn(db, "devices", "cpu_usage", "REAL DEFAULT 0")
	ensureColumn(db, "devices", "ram_total", "INTEGER DEFAULT 0")
	ensureColumn(db, "devices", "ram_used", "INTEGER DEFAULT 0")
	ensureColumn(db, "devices", "ram_usage_percent", "REAL DEFAULT 0")
	ensureColumn(db, "devices", "disk_total", "INTEGER DEFAULT 0")
	ensureColumn(db, "devices", "disk_used", "INTEGER DEFAULT 0")
	ensureColumn(db, "devices", "disk_free", "INTEGER DEFAULT 0")
	ensureColumn(db, "devices", "disk_free_human", "TEXT")
	ensureColumn(db, "devices", "last_seen", "TEXT")
	ensureColumn(db, "devices", "approved", "INTEGER DEFAULT 0")

	ensureColumn(db, "actions", "created_at", "TEXT")
	ensureColumn(db, "actions", "updated_at", "TEXT")
	ensureColumn(db, "history", "created_at", "TEXT")
	ensureColumn(db, "users", "password_hash", "TEXT")

	mustExec(db, `UPDATE devices SET approved = 0 WHERE approved IS NULL;`)
}

func mustExec(db *sql.DB, query string) {
	if _, err := db.Exec(query); err != nil {
		log.Fatal(err)
	}
}

func ensureColumn(db *sql.DB, table, column, colType string) {
	rows, err := db.Query(fmt.Sprintf("PRAGMA table_info(%s)", table))
	if err != nil {
		log.Fatal(err)
	}
	defer rows.Close()

	found := false

	for rows.Next() {
		var cid int
		var name, ctype string
		var notnull int
		var dflt sql.NullString
		var pk int

		if err := rows.Scan(&cid, &name, &ctype, &notnull, &dflt, &pk); err != nil {
			log.Fatal(err)
		}

		if strings.EqualFold(name, column) {
			found = true
			break
		}
	}

	if err := rows.Err(); err != nil {
		log.Fatal(err)
	}

	if found {
		return
	}

	alter := fmt.Sprintf("ALTER TABLE %s ADD COLUMN %s %s", table, column, colType)
	if _, err := db.Exec(alter); err != nil {
		log.Fatal(err)
	}
}