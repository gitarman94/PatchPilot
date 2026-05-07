package main

import (
	"crypto/rand"
	"database/sql"
	"encoding/base64"
	"net/http"

	"golang.org/x/crypto/bcrypt"
)

const sessionCookie = "commandpilot_session"

func randomToken() (string, error) {
	b := make([]byte, 32)
	if _, err := rand.Read(b); err != nil {
		return "", err
	}
	return base64.RawURLEncoding.EncodeToString(b), nil
}

func (a *App) currentUsername(r *http.Request) (string, bool) {
	cookie, err := r.Cookie(sessionCookie)
	if err != nil || cookie.Value == "" {
		return "", false
	}

	var username string
	err = a.DB.QueryRow(
		`SELECT username FROM sessions WHERE token = ?`,
		cookie.Value,
	).Scan(&username)
	if err != nil {
		return "", false
	}

	return username, true
}

func (a *App) login(w http.ResponseWriter, r *http.Request) {
	if r.Method == http.MethodGet {
		a.renderTemplate(w, "login.html", nil)
		return
	}

	username := r.FormValue("username")
	password := r.FormValue("password")

	var hash string
	err := a.DB.QueryRow(
		"SELECT password_hash FROM users WHERE username = ?",
		username,
	).Scan(&hash)
	if err != nil {
		if err == sql.ErrNoRows {
			http.Error(w, "Invalid credentials", http.StatusUnauthorized)
			return
		}
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	if bcrypt.CompareHashAndPassword([]byte(hash), []byte(password)) != nil {
		http.Error(w, "Invalid credentials", http.StatusUnauthorized)
		return
	}

	token, err := randomToken()
	if err != nil {
		http.Error(w, "Failed to create session", http.StatusInternalServerError)
		return
	}

	_, err = a.DB.Exec(
		`INSERT INTO sessions (token, username) VALUES (?, ?)`,
		token,
		username,
	)
	if err != nil {
		http.Error(w, "Failed to persist session", http.StatusInternalServerError)
		return
	}

	http.SetCookie(w, &http.Cookie{
		Name:     sessionCookie,
		Value:    token,
		Path:     "/",
		HttpOnly: true,
		SameSite: http.SameSiteLaxMode,
		Secure:   r.TLS != nil,
	})

	http.Redirect(w, r, "/dashboard", http.StatusFound)
}

func (a *App) logout(w http.ResponseWriter, r *http.Request) {
	if cookie, err := r.Cookie(sessionCookie); err == nil && cookie.Value != "" {
		_, _ = a.DB.Exec(`DELETE FROM sessions WHERE token = ?`, cookie.Value)
	}

	http.SetCookie(w, &http.Cookie{
		Name:     sessionCookie,
		Value:    "",
		Path:     "/",
		MaxAge:   -1,
		HttpOnly: true,
		SameSite: http.SameSiteLaxMode,
		Secure:   r.TLS != nil,
	})

	http.Redirect(w, r, "/", http.StatusFound)
}

func (a *App) requireAuth(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		if _, ok := a.currentUsername(r); !ok {
			http.Redirect(w, r, "/", http.StatusFound)
			return
		}
		next(w, r)
	}
}

func (a *App) changePassword(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/settings_page", http.StatusSeeOther)
		return
	}

	username, ok := a.currentUsername(r)
	if !ok {
		http.Redirect(w, r, "/", http.StatusFound)
		return
	}

	currentPassword := r.FormValue("current_password")
	newPassword := r.FormValue("new_password")
	confirmPassword := r.FormValue("confirm_password")

	if currentPassword == "" || newPassword == "" || confirmPassword == "" {
		http.Redirect(w, r, "/settings_page?error=missing", http.StatusSeeOther)
		return
	}

	if newPassword != confirmPassword {
		http.Redirect(w, r, "/settings_page?error=nomatch", http.StatusSeeOther)
		return
	}

	if len(newPassword) < 8 {
		http.Redirect(w, r, "/settings_page?error=short", http.StatusSeeOther)
		return
	}

	var hash string
	err := a.DB.QueryRow(
		`SELECT password_hash FROM users WHERE username = ?`,
		username,
	).Scan(&hash)
	if err != nil {
		http.Redirect(w, r, "/settings_page?error=notfound", http.StatusSeeOther)
		return
	}

	if bcrypt.CompareHashAndPassword([]byte(hash), []byte(currentPassword)) != nil {
		http.Redirect(w, r, "/settings_page?error=current", http.StatusSeeOther)
		return
	}

	newHash, err := bcrypt.GenerateFromPassword([]byte(newPassword), bcrypt.DefaultCost)
	if err != nil {
		http.Redirect(w, r, "/settings_page?error=hash", http.StatusSeeOther)
		return
	}

	_, err = a.DB.Exec(
		`UPDATE users SET password_hash = ? WHERE username = ?`,
		string(newHash),
		username,
	)
	if err != nil {
		http.Redirect(w, r, "/settings_page?error=update", http.StatusSeeOther)
		return
	}

	http.Redirect(w, r, "/settings_page?password=changed", http.StatusSeeOther)
}