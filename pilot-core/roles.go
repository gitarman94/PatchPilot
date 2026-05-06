package main

import "net/http"

func (app *App) rolesPage(w http.ResponseWriter, r *http.Request) {
	rows, err := app.DB.Query("SELECT id, name FROM roles")
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	var roles []Role
	for rows.Next() {
		var role Role
		if err := rows.Scan(&role.ID, &role.Name); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
		roles = append(roles, role)
	}

	app.renderTemplate(w, "roles.html", roles)
}

func (app *App) createRole(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Redirect(w, r, "/roles_page", http.StatusSeeOther)
		return
	}

	name := r.FormValue("name")
	if name == "" {
		http.Error(w, "Missing role name", http.StatusBadRequest)
		return
	}

	_, err := app.DB.Exec("INSERT INTO roles (name) VALUES (?)", name)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	http.Redirect(w, r, "/roles_page", http.StatusSeeOther)
}