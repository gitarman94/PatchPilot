package main

import (
	"net/http"
)

type User struct {
	ID       int
	Username string
	RoleID   int
}

type Group struct {
	ID   int
	Name string
}

func (a *App) usersPage(w http.ResponseWriter, r *http.Request) {
	rows, _ := a.DB.Query("SELECT id, username, role_id FROM users")

	var users []User
	for rows.Next() {
		var u User
		rows.Scan(&u.ID, &u.Username, &u.RoleID)
		users = append(users, u)
	}

	a.Templates.ExecuteTemplate(w, "users.html", map[string]interface{}{
		"Users": users,
	})
}

func (a *App) createUser(w http.ResponseWriter, r *http.Request) {
	username := r.FormValue("username")
	password := r.FormValue("password")

	hash, _ := bcrypt.GenerateFromPassword([]byte(password), bcrypt.DefaultCost)

	a.DB.Exec("INSERT INTO users (username, password) VALUES (?, ?)", username, hash)

	http.Redirect(w, r, "/users_page", http.StatusFound)
}

func (a *App) groupsPage(w http.ResponseWriter, r *http.Request) {
	rows, _ := a.DB.Query("SELECT id, name FROM groups")

	var groups []Group
	for rows.Next() {
		var g Group
		rows.Scan(&g.ID, &g.Name)
		groups = append(groups, g)
	}

	a.Templates.ExecuteTemplate(w, "groups.html", map[string]interface{}{
		"Groups": groups,
	})
}