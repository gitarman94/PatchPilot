package main

import (
	"database/sql"
	"encoding/json"
	"net/http"
	"strconv"
	"strings"
	"time"
)

type rowScanner interface {
	Scan(dest ...any) error
}

func scanDevice(scanner rowScanner) (Device, error) {
	var d Device
	var approved int

	err := scanner.Scan(
		&d.ID,
		&d.AgentID,
		&d.Hostname,
		&d.FQDN,
		&d.IP,
		&d.OS,
		&d.Architecture,
		&d.DeviceType,
		&d.DeviceModel,
		&d.CPUModel,
		&d.CPUUsage,
		&d.RAMTotal,
		&d.RAMUsed,
		&d.RAMUsagePercent,
		&d.DiskTotal,
		&d.DiskUsed,
		&d.DiskFree,
		&d.DiskFreeHuman,
		&d.LastSeen,
		&approved,
	)
	if err != nil {
		return Device{}, err
	}

	d.Approved = approved != 0
	return d, nil
}

func scanAction(scanner rowScanner) (Action, error) {
	var act Action
	err := scanner.Scan(
		&act.ID,
		&act.Name,
		&act.DeviceID,
		&act.Status,
		&act.CreatedAt,
		&act.UpdatedAt,
	)
	if err != nil {
		return Action{}, err
	}
	return act, nil
}

func loadRecentDevices(db *sql.DB, limit int) ([]Device, error) {
	query := `
SELECT
	id,
	IFNULL(agent_id, ''),
	IFNULL(hostname, ''),
	IFNULL(fqdn, ''),
	IFNULL(ip, ''),
	IFNULL(os, ''),
	IFNULL(architecture, ''),
	IFNULL(device_type, ''),
	IFNULL(device_model, ''),
	IFNULL(cpu_model, ''),
	IFNULL(cpu_usage, 0),
	IFNULL(ram_total, 0),
	IFNULL(ram_used, 0),
	IFNULL(ram_usage_percent, 0),
	IFNULL(disk_total, 0),
	IFNULL(disk_used, 0),
	IFNULL(disk_free, 0),
	IFNULL(disk_free_human, ''),
	IFNULL(last_seen, ''),
	approved
FROM devices
ORDER BY COALESCE(last_seen, '') DESC, id DESC
`
	args := []any{}
	if limit > 0 {
		query += " LIMIT ?"
		args = append(args, limit)
	}

	rows, err := db.Query(query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var devices []Device
	for rows.Next() {
		d, err := scanDevice(rows)
		if err != nil {
			continue
		}
		devices = append(devices, d)
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	return devices, nil
}

func loadRecentActions(db *sql.DB, limit int) ([]Action, error) {
	query := `
SELECT
	id,
	IFNULL(name, ''),
	IFNULL(device_id, 0),
	IFNULL(status, ''),
	IFNULL(created_at, ''),
	IFNULL(updated_at, '')
FROM actions
ORDER BY id DESC
`
	args := []any{}
	if limit > 0 {
		query += " LIMIT ?"
		args = append(args, limit)
	}

	rows, err := db.Query(query, args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var actions []Action
	for rows.Next() {
		act, err := scanAction(rows)
		if err != nil {
			continue
		}
		actions = append(actions, act)
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	return actions, nil
}

func loadDeviceByID(db *sql.DB, id int) (Device, error) {
	row := db.QueryRow(`
SELECT
	id,
	IFNULL(agent_id, ''),
	IFNULL(hostname, ''),
	IFNULL(fqdn, ''),
	IFNULL(ip, ''),
	IFNULL(os, ''),
	IFNULL(architecture, ''),
	IFNULL(device_type, ''),
	IFNULL(device_model, ''),
	IFNULL(cpu_model, ''),
	IFNULL(cpu_usage, 0),
	IFNULL(ram_total, 0),
	IFNULL(ram_used, 0),
	IFNULL(ram_usage_percent, 0),
	IFNULL(disk_total, 0),
	IFNULL(disk_used, 0),
	IFNULL(disk_free, 0),
	IFNULL(disk_free_human, ''),
	IFNULL(last_seen, ''),
	approved
FROM devices
WHERE id = ?
`, id)

	return scanDevice(row)
}

func (a *App) devicesPage(w http.ResponseWriter, r *http.Request) {
	devices, err := loadRecentDevices(a.DB, 0)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	data := DevicesPageData{
		Devices:         devices,
		TotalDevices:     queryCount(a.DB, `SELECT COUNT(*) FROM devices`),
		ApprovedDevices:  queryCount(a.DB, `SELECT COUNT(*) FROM devices WHERE approved = 1`),
		PendingDevices:   queryCount(a.DB, `SELECT COUNT(*) FROM devices WHERE approved = 0`),
		OnlineDevices:    queryCount(a.DB, `SELECT COUNT(*) FROM devices WHERE last_seen IS NOT NULL AND last_seen <> '' AND datetime(last_seen) >= datetime('now', '-5 minutes')`),
	}

	a.Templates.ExecuteTemplate(w, "devices.html", data)
}

func (a *App) deviceDetail(w http.ResponseWriter, r *http.Request) {
	idStr := strings.TrimPrefix(r.URL.Path, "/device_detail/")
	id, err := strconv.Atoi(idStr)
	if err != nil {
		http.NotFound(w, r)
		return
	}

	d, err := loadDeviceByID(a.DB, id)
	if err == sql.ErrNoRows {
		a.Templates.ExecuteTemplate(w, "device_detail.html", DeviceDetailPageData{})
		return
	}
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	a.Templates.ExecuteTemplate(w, "device_detail.html", DeviceDetailPageData{Device: &d})
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

	_, err = tx.Exec("UPDATE devices SET approved = 1, last_seen = ? WHERE id = ?", time.Now().UTC().Format(time.RFC3339), id)
	if err != nil {
		_ = tx.Rollback()
		http.Error(w, err.Error(), 500)
		return
	}

	if err := tx.Commit(); err != nil {
		http.Error(w, err.Error(), 500)
		return
	}

	_, _ = a.DB.Exec(`INSERT INTO history (action, device_id, created_at) VALUES (?, ?, ?)`, "device_approved", id, time.Now().UTC().Format(time.RFC3339))
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
		_ = tx.Rollback()
		http.Error(w, err.Error(), 500)
		return
	}

	if err := tx.Commit(); err != nil {
		http.Error(w, err.Error(), 500)
		return
	}

	_, _ = a.DB.Exec(`INSERT INTO history (action, device_id, created_at) VALUES (?, ?, ?)`, "device_rejected", id, time.Now().UTC().Format(time.RFC3339))
	http.Redirect(w, r, "/devices_page", http.StatusFound)
}

func (a *App) apiDevices(w http.ResponseWriter, r *http.Request) {
	devices, err := loadRecentDevices(a.DB, 0)
	if err != nil {
		http.Error(w, err.Error(), 500)
		return
	}
	writeJSON(w, devices)
}

func (a *App) agentCheckinHandler(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req AgentCheckinRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}

	req.AgentID = strings.TrimSpace(req.AgentID)
	req.Hostname = strings.TrimSpace(req.Hostname)
	req.FQDN = strings.TrimSpace(req.FQDN)
	req.IP = strings.TrimSpace(req.IP)
	req.OS = strings.TrimSpace(req.OS)
	req.Architecture = strings.TrimSpace(req.Architecture)
	req.DeviceType = strings.TrimSpace(req.DeviceType)
	req.DeviceModel = strings.TrimSpace(req.DeviceModel)
	req.CPUModel = strings.TrimSpace(req.CPUModel)
	req.DiskFreeHuman = strings.TrimSpace(req.DiskFreeHuman)
	req.Version = strings.TrimSpace(req.Version)

	if req.AgentID == "" {
		http.Error(w, "agent_id is required", http.StatusBadRequest)
		return
	}

	now := time.Now().UTC().Format(time.RFC3339)

	var id int
	var approved int
	err := a.DB.QueryRow(`SELECT id, approved FROM devices WHERE agent_id = ? LIMIT 1`, req.AgentID).Scan(&id, &approved)

	switch err {
	case sql.ErrNoRows:
		res, err := a.DB.Exec(`
INSERT INTO devices (
	agent_id,
	hostname,
	fqdn,
	ip,
	os,
	architecture,
	device_type,
	device_model,
	cpu_model,
	cpu_usage,
	ram_total,
	ram_used,
	ram_usage_percent,
	disk_total,
	disk_used,
	disk_free,
	disk_free_human,
	last_seen,
	approved
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 0)
`, req.AgentID, req.Hostname, req.FQDN, req.IP, req.OS, req.Architecture, req.DeviceType, req.DeviceModel, req.CPUModel, req.CPUUsage, req.RAMTotal, req.RAMUsed, req.RAMUsagePercent, req.DiskTotal, req.DiskUsed, req.DiskFree, req.DiskFreeHuman, now)
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		lastID, _ := res.LastInsertId()
		id = int(lastID)
		approved = 0
	case nil:
		_, err = a.DB.Exec(`
UPDATE devices
SET
	hostname = ?,
	fqdn = ?,
	ip = ?,
	os = ?,
	architecture = ?,
	device_type = ?,
	device_model = ?,
	cpu_model = ?,
	cpu_usage = ?,
	ram_total = ?,
	ram_used = ?,
	ram_usage_percent = ?,
	disk_total = ?,
	disk_used = ?,
	disk_free = ?,
	disk_free_human = ?,
	last_seen = ?
WHERE agent_id = ?
`, req.Hostname, req.FQDN, req.IP, req.OS, req.Architecture, req.DeviceType, req.DeviceModel, req.CPUModel, req.CPUUsage, req.RAMTotal, req.RAMUsed, req.RAMUsagePercent, req.DiskTotal, req.DiskUsed, req.DiskFree, req.DiskFreeHuman, now, req.AgentID)
		if err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
	default:
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	if err == nil {
		_ = a.DB.QueryRow(`SELECT id, approved FROM devices WHERE agent_id = ? LIMIT 1`, req.AgentID).Scan(&id, &approved)
	}

	writeJSON(w, AgentCheckinResponse{
		Status:   "ok",
		DeviceID: id,
		Approved: approved != 0,
	})
}