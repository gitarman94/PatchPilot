package main

import (
	"net/http"

	"golang.org/x/crypto/bcrypt"
)

const sessionCookie = "kentro_session"

func (a *App) login(w http.ResponseWriter, r *http.Request) {
	if r.Method == http.MethodGet {
		a.Templates.ExecuteTemplate(w, "login.html", nil)
		return
	}

	username := r.FormValue("username")
	password := r.FormValue("password")

	var hash string
	err := a.DB.QueryRow("SELECT password FROM users WHERE username=?", username).Scan(&hash)
	if err != nil {
		http.Error(w, "Invalid credentials", 401)
		return
	}

	if bcrypt.CompareHashAndPassword([]byte(hash), []byte(password)) != nil {
		http.Error(w, "Invalid credentials", 401)
		return
	}

	http.SetCookie(w, &http.Cookie{
		Name:     sessionCookie,
		Value:    username,
		Path:     "/",
		HttpOnly: true,
	})

	http.Redirect(w, r, "/dashboard", http.StatusFound)
}

func (a *App) logout(w http.ResponseWriter, r *http.Request) {
	http.SetCookie(w, &http.Cookie{
		Name:   sessionCookie,
		Value:  "",
		Path:   "/",
		MaxAge: -1,
	})
	http.Redirect(w, r, "/", http.StatusFound)
}

func (a *App) requireAuth(next http.HandlerFunc) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		cookie, err := r.Cookie(sessionCookie)
		if err != nil || cookie.Value == "" {
			http.Redirect(w, r, "/auth/login", http.StatusFound)
			return
		}
		next(w, r)
	}
}