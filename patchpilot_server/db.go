package main

import "database/sql"

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
		timestamp TEXT
	)`)

	db.Exec(`CREATE TABLE IF NOT EXISTS users (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		username TEXT UNIQUE,
		password TEXT,
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
}