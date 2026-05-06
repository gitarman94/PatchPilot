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
		http.Error(w, err.Error(), 500)
		return
	}
	defer rows.Close()

	var history []History

	for rows.Next() {
		var h History
		rows.Scan(&h.ID, &h.Action, &h.Timestamp)
		history = append(history, h)
	}

	a.Templates.ExecuteTemplate(w, "history.html", map[string]interface{}{
		"history": history,
	})
}

func (a *App) logHistory(action string) {
	a.DB.Exec(`
		INSERT INTO history (action, timestamp)
		VALUES (?, ?)
	`, action, time.Now().Format(time.RFC3339))
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
		rows.Scan(&h.ID, &h.Action, &h.Timestamp)
		history = append(history, h)
	}

	writeJSON(w, history)
}