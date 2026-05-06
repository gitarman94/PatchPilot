package main

import "net/http"

func (app *App) settingsPage(w http.ResponseWriter, r *http.Request) {
	rows, err := app.DB.Query("SELECT key, value FROM settings")
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	type SettingKV struct {
		Key   string
		Value string
	}

	var settings []SettingKV
	for rows.Next() {
		var s SettingKV
		if err := rows.Scan(&s.Key, &s.Value); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		settings = append(settings, s)
	}

	app.renderTemplate(w, "settings.html", settings)
}

func (app *App) updateSetting(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/settings_page", http.StatusSeeOther)
		return
	}

	key := r.FormValue("key")
	value := r.FormValue("value")

	_, err := app.DB.Exec("UPDATE settings SET value=? WHERE key=?", value, key)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	http.Redirect(w, r, "/settings_page", http.StatusSeeOther)
}