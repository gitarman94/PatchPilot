package main

import (
	"database/sql"
	"strings"

	"golang.org/x/crypto/bcrypt"
)

func initDB(db *sql.DB) {

	db.Exec(`CREATE TABLE IF NOT EXISTS devices (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		hostname TEXT,
		ip TEXT,
		os TEXT,
		last_seen TEXT,
		approved INTEGER DEFAULT 0
	)`)

	db.Exec(`CREATE TABLE IF NOT EXISTS actions (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		name TEXT,
		device_id INTEGER,
		status TEXT,
		created_at TEXT,
		updated_at TEXT
	)`)

	db.Exec(`CREATE TABLE IF NOT EXISTS history (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		action TEXT,
		device_id INTEGER DEFAULT 0,
		created_at TEXT
	)`)

	db.Exec(`CREATE TABLE IF NOT EXISTS users (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		username TEXT UNIQUE,
		password_hash TEXT,
		role_id INTEGER
	)`)

	db.Exec(`CREATE TABLE IF NOT EXISTS roles (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		name TEXT UNIQUE
	)`)

	db.Exec(`CREATE TABLE IF NOT EXISTS groups (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		name TEXT UNIQUE
	)`)

	db.Exec(`CREATE TABLE IF NOT EXISTS user_groups (
		user_id INTEGER,
		group_id INTEGER
	)`)

	db.Exec(`CREATE TABLE IF NOT EXISTS settings (
		key TEXT PRIMARY KEY,
		value TEXT
	)`)

	_, _ = db.Exec(`ALTER TABLE users ADD COLUMN password_hash TEXT`)
	_, _ = db.Exec(`ALTER TABLE history ADD COLUMN device_id INTEGER DEFAULT 0`)
	_, _ = db.Exec(`ALTER TABLE history ADD COLUMN created_at TEXT`)

	rows, err := db.Query(`PRAGMA table_info(history)`)
	if err == nil {
		defer rows.Close()

		hasTimestamp := false
		hasCreatedAt := false

		for rows.Next() {
			var cid int
			var name string
			var ctype string
			var notnull int
			var dflt sql.NullString
			var pk int

			if rows.Scan(&cid, &name, &ctype, &notnull, &dflt, &pk) == nil {
				switch strings.ToLower(name) {
				case "timestamp":
					hasTimestamp = true
				case "created_at":
					hasCreatedAt = true
				}
			}
		}

		if hasTimestamp && hasCreatedAt {
			_, _ = db.Exec(`
				UPDATE history
				SET created_at = timestamp
				WHERE (created_at IS NULL OR created_at = '')
				AND timestamp IS NOT NULL
			`)
		}
	}

	rows, err = db.Query(`
		SELECT id, password
		FROM users
		WHERE password IS NOT NULL
		AND (password_hash IS NULL OR password_hash = '')
	`)
	if err == nil {
		defer rows.Close()

		for rows.Next() {
			var id int
			var password string

			if err := rows.Scan(&id, &password); err == nil {
				hashed, err := bcrypt.GenerateFromPassword([]byte(password), bcrypt.DefaultCost)
				if err == nil {
					db.Exec(
						`UPDATE users SET password_hash = ? WHERE id = ?`,
						string(hashed),
						id,
					)
				}
			}
		}
	}
}