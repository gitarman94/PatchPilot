package main

import (
	"net/http"
	"strconv"
)

func (a *App) devicesPage(w http.ResponseWriter, r *http.Request) {
	rows, err := a.DB.Query(`
		SELECT id, hostname, ip, os, last_seen, approved
		FROM devices ORDER BY id DESC
	`)
	if err != nil {
		http.Error(w, err.Error(), 500)
		return
	}
	defer rows.Close()

	var devices []Device

	for rows.Next() {
		var d Device
		rows.Scan(&d.ID, &d.Hostname, &d.IP, &d.OS, &d.LastSeen, &d.Approved)
		devices = append(devices, d)
	}

	a.Templates.ExecuteTemplate(w, "devices.html", map[string]interface{}{
		"devices": devices,
	})
}

func (a *App) deviceDetail(w http.ResponseWriter, r *http.Request) {
	idStr := r.URL.Path[len("/device_detail/"):]
	id, _ := strconv.Atoi(idStr)

	var d Device
	err := a.DB.QueryRow(`
		SELECT id, hostname, ip, os, last_seen, approved
		FROM devices WHERE id = ?
	`, id).Scan(&d.ID, &d.Hostname, &d.IP, &d.OS, &d.LastSeen, &d.Approved)

	if err != nil {
		http.NotFound(w, r)
		return
	}

	a.Templates.ExecuteTemplate(w, "device_detail.html", d)
}

func (a *App) approveDevice(w http.ResponseWriter, r *http.Request) {
	id, _ := strconv.Atoi(r.URL.Query().Get("id"))

	a.DB.Exec("UPDATE devices SET approved = 1 WHERE id = ?", id)

	a.logHistory("device_approved")

	http.Redirect(w, r, "/devices_page", http.StatusFound)
}

func (a *App) rejectDevice(w http.ResponseWriter, r *http.Request) {
	id, _ := strconv.Atoi(r.URL.Query().Get("id"))

	a.DB.Exec("DELETE FROM devices WHERE id = ?", id)

	a.logHistory("device_rejected")

	http.Redirect(w, r, "/devices_page", http.StatusFound)
}

/* --- API --- */

func (a *App) apiDevices(w http.ResponseWriter, r *http.Request) {
	rows, err := a.DB.Query(`
		SELECT id, hostname, ip, os, last_seen, approved FROM devices
	`)
	if err != nil {
		http.Error(w, err.Error(), 500)
		return
	}
	defer rows.Close()

	var out []Device

	for rows.Next() {
		var d Device
		rows.Scan(&d.ID, &d.Hostname, &d.IP, &d.OS, &d.LastSeen, &d.Approved)
		out = append(out, d)
	}

	writeJSON(w, out)
}