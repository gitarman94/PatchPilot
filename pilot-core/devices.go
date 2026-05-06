package main

import (
	"net/http"
	"strconv"
	"strings"
)

func (a *App) devicesPage(w http.ResponseWriter, r *http.Request) {
	rows, err := a.DB.Query(`
		SELECT id, hostname, ip, os, IFNULL(last_seen, '') , approved
		FROM devices ORDER BY id DESC
	`)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	var devices []Device

	for rows.Next() {
		var d Device

		err := rows.Scan(&d.ID, &d.Hostname, &d.IP, &d.OS, &d.LastSeen, &d.Approved)
		if err != nil {
			continue
		}

		devices = append(devices, d)
	}

	if err := rows.Err(); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	a.Templates.ExecuteTemplate(w, "devices.html", map[string]interface{}{
		"Devices": devices,
	})
}

func (a *App) deviceDetail(w http.ResponseWriter, r *http.Request) {
	idStr := strings.TrimPrefix(r.URL.Path, "/device_detail/")
	id, err := strconv.Atoi(idStr)
	if err != nil {
		http.NotFound(w, r)
		return
	}

	var d Device

	err = a.DB.QueryRow(`
		SELECT id, hostname, ip, os, IFNULL(last_seen, ''), approved
		FROM devices WHERE id = ?
	`, id).Scan(&d.ID, &d.Hostname, &d.IP, &d.OS, &d.LastSeen, &d.Approved)

	if err != nil {
		http.NotFound(w, r)
		return
	}

	a.Templates.ExecuteTemplate(w, "device_detail.html", map[string]interface{}{
		"Device": d,
	})
}

func (a *App) approveDevice(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}

	id, err := strconv.Atoi(r.FormValue("id"))
	if err != nil {
		http.Error(w, "invalid id", http.StatusBadRequest)
		return
	}

	tx, err := a.DB.Begin()
	if err != nil {
		http.Error(w, err.Error(), 500)
		return
	}

	_, err = tx.Exec("UPDATE devices SET approved = 1, last_seen = datetime('now') WHERE id = ?", id)
	if err != nil {
		tx.Rollback()
		http.Error(w, err.Error(), 500)
		return
	}

	if err := tx.Commit(); err != nil {
		http.Error(w, err.Error(), 500)
		return
	}

	// log with device_id (matches new history schema)
	_, _ = a.DB.Exec(`
		INSERT INTO history (action, device_id, created_at)
		VALUES (?, ?, datetime('now'))
	`, "device_approved", id)

	http.Redirect(w, r, "/devices_page", http.StatusFound)
}

func (a *App) rejectDevice(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}

	id, err := strconv.Atoi(r.FormValue("id"))
	if err != nil {
		http.Error(w, "invalid id", http.StatusBadRequest)
		return
	}

	tx, err := a.DB.Begin()
	if err != nil {
		http.Error(w, err.Error(), 500)
		return
	}

	_, err = tx.Exec("DELETE FROM devices WHERE id = ?", id)
	if err != nil {
		tx.Rollback()
		http.Error(w, err.Error(), 500)
		return
	}

	if err := tx.Commit(); err != nil {
		http.Error(w, err.Error(), 500)
		return
	}

	// log with device_id
	_, _ = a.DB.Exec(`
		INSERT INTO history (action, device_id, created_at)
		VALUES (?, ?, datetime('now'))
	`, "device_rejected", id)

	http.Redirect(w, r, "/devices_page", http.StatusFound)
}

func (a *App) apiDevices(w http.ResponseWriter, r *http.Request) {
	rows, err := a.DB.Query(`
		SELECT id, hostname, ip, os, IFNULL(last_seen, ''), approved FROM devices
	`)
	if err != nil {
		http.Error(w, err.Error(), 500)
		return
	}
	defer rows.Close()

	var out []Device

	for rows.Next() {
		var d Device

		err := rows.Scan(&d.ID, &d.Hostname, &d.IP, &d.OS, &d.LastSeen, &d.Approved)
		if err != nil {
			continue
		}

		out = append(out, d)
	}

	if err := rows.Err(); err != nil {
		http.Error(w, err.Error(), 500)
		return
	}

	writeJSON(w, out)
}