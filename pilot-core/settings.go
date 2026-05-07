package main

import (
	"net/http"
	"strings"
)

type settingsPageData struct {
	Settings        []Setting
	PasswordChanged string
	PasswordError   string
}

func (app *App) settingsPage(w http.ResponseWriter, r *http.Request) {
	rows, err := app.DB.Query("SELECT key, value FROM settings ORDER BY key")
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	var settings []Setting
	for rows.Next() {
		var s Setting
		if err := rows.Scan(&s.Key, &s.Value); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		settings = append(settings, s)
	}

	data := settingsPageData{
		Settings: settings,
	}

	if r.URL.Query().Get("password") == "changed" {
		data.PasswordChanged = "true"
	}

	switch r.URL.Query().Get("error") {
	case "missing":
		data.PasswordError = "All password fields are required."
	case "nomatch":
		data.PasswordError = "New passwords do not match."
	case "short":
		data.PasswordError = "New password must be at least 8 characters."
	case "current":
		data.PasswordError = "Current password is incorrect."
	case "notfound":
		data.PasswordError = "User not found."
	case "hash":
		data.PasswordError = "Failed to hash new password."
	case "update":
		data.PasswordError = "Failed to update password."
	}

	app.renderTemplate(w, "settings.html", data)
}

func (app *App) updateSetting(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/settings_page", http.StatusSeeOther)
		return
	}

	key := strings.TrimSpace(r.FormValue("key"))
	value := r.FormValue("value")

	if key == "" {
		http.Error(w, "Missing configuration key", http.StatusBadRequest)
		return
	}

	_, err := app.DB.Exec(`
		INSERT INTO settings (key, value)
		VALUES (?, ?)
		ON CONFLICT(key) DO UPDATE SET value=excluded.value
	`, key, value)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	http.Redirect(w, r, "/settings_page", http.StatusSeeOther)
}