package main

import "net/http"

type Setting struct {
	Key   string
	Value string
}

func (a *App) settingsPage(w http.ResponseWriter, r *http.Request) {
	rows, _ := a.DB.Query("SELECT key, value FROM settings")

	var settings []Setting
	for rows.Next() {
		var s Setting
		rows.Scan(&s.Key, &s.Value)
		settings = append(settings, s)
	}

	a.Templates.ExecuteTemplate(w, "settings.html", map[string]interface{}{
		"Settings": settings,
	})
}

func (a *App) updateSetting(w http.ResponseWriter, r *http.Request) {
	key := r.FormValue("key")
	value := r.FormValue("value")

	a.DB.Exec(`
		INSERT INTO settings (key, value)
		VALUES (?, ?)
		ON CONFLICT(key) DO UPDATE SET value=excluded.value
	`, key, value)

	http.Redirect(w, r, "/settings_page", http.StatusFound)
}