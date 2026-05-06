package main

import (
	"net/http"
	"strconv"
	"time"
)

func (a *App) actionsPage(w http.ResponseWriter, r *http.Request) {
	rows, err := a.DB.Query(`
		SELECT id, name, status, device_id, created_at, updated_at
		FROM actions
		ORDER BY id DESC
	`)
	if err != nil {
		http.Error(w, err.Error(), 500)
		return
	}
	defer rows.Close()

	var actions []Action

	for rows.Next() {
		var act Action
		rows.Scan(
			&act.ID,
			&act.Name,
			&act.Status,
			&act.DeviceID,
			&act.CreatedAt,
			&act.UpdatedAt,
		)
		actions = append(actions, act)
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	a.Templates.ExecuteTemplate(w, "actions.html", map[string]interface{}{
		"actions": actions,
	})
}

func (a *App) submitAction(w http.ResponseWriter, r *http.Request) {
	deviceID, _ := strconv.Atoi(r.FormValue("device_id"))
	name := r.FormValue("name")

	now := time.Now().Format(time.RFC3339)

	_, err := a.DB.Exec(`
		INSERT INTO actions (name, device_id, status, created_at, updated_at)
		VALUES (?, ?, 'pending', ?, ?)
	`, name, deviceID, now, now)

	if err != nil {
		http.Error(w, err.Error(), 500)
		return
	}

	a.logHistory("action_created")

	http.Redirect(w, r, "/actions_page", http.StatusFound)
}

func (a *App) updateActionStatus(w http.ResponseWriter, r *http.Request) {
	id, _ := strconv.Atoi(r.FormValue("id"))
	status := r.FormValue("status")

	now := time.Now().Format(time.RFC3339)

	_, err := a.DB.Exec(`
		UPDATE actions
		SET status = ?, updated_at = ?
		WHERE id = ?
	`, status, now, id)

	if err != nil {
		http.Error(w, err.Error(), 500)
		return
	}

	a.logHistory("action_updated")

	w.WriteHeader(200)
}

/* --- API --- */

func (a *App) apiActions(w http.ResponseWriter, r *http.Request) {
	rows, err := a.DB.Query(`
		SELECT id, name, status, device_id, created_at, updated_at
		FROM actions
		ORDER BY id DESC
	`)
	if err != nil {
		http.Error(w, err.Error(), 500)
		return
	}
	defer rows.Close()

	var out []Action

	for rows.Next() {
		var act Action
		rows.Scan(
			&act.ID,
			&act.Name,
			&act.Status,
			&act.DeviceID,
			&act.CreatedAt,
			&act.UpdatedAt,
		)
		out = append(out, act)
	}

	writeJSON(w, out)
}