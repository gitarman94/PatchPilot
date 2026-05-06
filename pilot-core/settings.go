package main

import (
	"net/http"
)

func (app *App) settingsHandler(w http.ResponseWriter, r *http.Request) {
	rows, err := app.DB.Query("SELECT id, key, value FROM settings")
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	var settings []Setting
	for rows.Next() {
		var s Setting
		if err := rows.Scan(&s.ID, &s.Key, &s.Value); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		settings = append(settings, s)
	}

	renderTemplate(w, "settings.html", settings)
}

func (app *App) updateSetting(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/settings", http.StatusSeeOther)
		return
	}

	id := r.FormValue("id")
	value := r.FormValue("value")

	_, err := app.DB.Exec("UPDATE settings SET value=? WHERE id=?", value, id)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	http.Redirect(w, r, "/settings", http.StatusSeeOther)
}