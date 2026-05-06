package main

import (
	"net/http"
	"time"
)

func (a *App) historyPage(w http.ResponseWriter, r *http.Request) {
	rows, err := a.DB.Query(`
		SELECT id, action, timestamp
		FROM history ORDER BY id DESC
	`)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	var history []History

	for rows.Next() {
		var h History
		err := rows.Scan(&h.ID, &h.Action, &h.Timestamp)
		if err != nil {
			continue
		}
		history = append(history, h)
	}

	if err := rows.Err(); err != nil {
		http.Error(w, err.Error(), 500)
		return
	}

	a.Templates.ExecuteTemplate(w, "history.html", map[string]interface{}{
		"History": history,
	})
}

func (a *App) logHistory(action string) {
	_, _ = a.DB.Exec(`
		INSERT INTO history (action, timestamp)
		VALUES (?, ?)
	`, action, time.Now())
}

func (a *App) apiHistory(w http.ResponseWriter, r *http.Request) {
	rows, err := a.DB.Query(`
		SELECT id, action, timestamp
		FROM history ORDER BY id DESC
	`)
	if err != nil {
		http.Error(w, err.Error(), 500)
		return
	}
	defer rows.Close()

	var history []History

	for rows.Next() {
		var h History
		err := rows.Scan(&h.ID, &h.Action, &h.Timestamp)
		if err != nil {
			continue
		}
		history = append(history, h)
	}

	if err := rows.Err(); err != nil {
		http.Error(w, err.Error(), 500)
		return
	}

	writeJSON(w, history)
}