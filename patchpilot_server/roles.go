package main

import "net/http"

type Role struct {
	ID   int
	Name string
}

func (a *App) rolesPage(w http.ResponseWriter, r *http.Request) {
	rows, _ := a.DB.Query("SELECT id, name FROM roles")

	var roles []Role
	for rows.Next() {
		var r2 Role
		rows.Scan(&r2.ID, &r2.Name)
		roles = append(roles, r2)
	}

	a.Templates.ExecuteTemplate(w, "roles.html", map[string]interface{}{
		"Roles": roles,
	})
}

func (a *App) createRole(w http.ResponseWriter, r *http.Request) {
	name := r.FormValue("name")

	a.DB.Exec("INSERT INTO roles (name) VALUES (?)", name)

	http.Redirect(w, r, "/roles_page", http.StatusFound)
}