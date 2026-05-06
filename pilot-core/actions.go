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
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	var actions []Action

	for rows.Next() {
		var act Action
		err := rows.Scan(
			&act.ID,
			&act.Name,
			&act.Status,
			&act.DeviceID,
			&act.CreatedAt,
			&act.UpdatedAt,
		)
		if err != nil {
			continue
		}
		actions = append(actions, act)
	}

	if err := rows.Err(); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	a.Templates.ExecuteTemplate(w, "actions.html", map[string]interface{}{
		"Actions": actions,
	})
}

func (a *App) submitAction(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}

	deviceID, err := strconv.Atoi(r.FormValue("device_id"))
	if err != nil {
		http.Error(w, "invalid device_id", http.StatusBadRequest)
		return
	}

	name := r.FormValue("name")
	if name == "" {
		http.Error(w, "name is required", http.StatusBadRequest)
		return
	}

	now := time.Now()

	_, err = a.DB.Exec(`
		INSERT INTO actions (name, device_id, status, created_at, updated_at)
		VALUES (?, ?, 'pending', ?, ?)
	`, name, deviceID, now, now)

	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	a.logHistory("action_created")

	http.Redirect(w, r, "/actions_page", http.StatusFound)
}

func (a *App) updateActionStatus(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}

	id, err := strconv.Atoi(r.FormValue("id"))
	if err != nil {
		http.Error(w, "invalid id", http.StatusBadRequest)
		return
	}

	status := r.FormValue("status")
	if status == "" {
		http.Error(w, "status is required", http.StatusBadRequest)
		return
	}

	now := time.Now()

	_, err = a.DB.Exec(`
		UPDATE actions
		SET status = ?, updated_at = ?
		WHERE id = ?
	`, status, now, id)

	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	a.logHistory("action_updated")

	w.WriteHeader(http.StatusOK)
}

func (a *App) apiActions(w http.ResponseWriter, r *http.Request) {
	rows, err := a.DB.Query(`
		SELECT id, name, status, device_id, created_at, updated_at
		FROM actions
		ORDER BY id DESC
	`)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	var out []Action

	for rows.Next() {
		var act Action
		err := rows.Scan(
			&act.ID,
			&act.Name,
			&act.Status,
			&act.DeviceID,
			&act.CreatedAt,
			&act.UpdatedAt,
		)
		if err != nil {
			continue
		}
		out = append(out, act)
	}

	if err := rows.Err(); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	writeJSON(w, out)
}