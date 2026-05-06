package main

import "net/http"

type Setting struct {
	Key   string
	Value string
}

func (a *App) settingsPage(w http.ResponseWriter, r *http.Request) {
	rows, err := a.DB.Query("SELECT key, value FROM settings")
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	var settings []Setting

	for rows.Next() {
		var s Setting
		if err := rows.Scan(&s.Key, &s.Value); err != nil {
			continue
		}
		settings = append(settings, s)
	}

	if err := rows.Err(); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	a.Templates.ExecuteTemplate(w, "settings.html", map[string]interface{}{
		"Settings": settings,
	})
}

func (a *App) updateSetting(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}

	key := r.FormValue("key")
	value := r.FormValue("value")

	if key == "" {
		http.Error(w, "key is required", http.StatusBadRequest)
		return
	}

	_, err := a.DB.Exec(`
		INSERT INTO settings (key, value)
		VALUES (?, ?)
		ON CONFLICT(key) DO UPDATE SET value=excluded.value
	`, key, value)

	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	http.Redirect(w, r, "/settings_page", http.StatusFound)
}