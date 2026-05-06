package main

import "net/http"

func (a *App) rolesPage(w http.ResponseWriter, r *http.Request) {
	rows, err := a.DB.Query("SELECT id, name, description FROM roles ORDER BY id DESC")
	if err != nil {
		http.Error(w, "Failed to load roles", http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	var roles []Role

	for rows.Next() {
		var role Role
		err := rows.Scan(&role.ID, &role.Name, &role.Description)
		if err != nil {
			continue
		}
		roles = append(roles, role)
	}

	if err := rows.Err(); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	a.Templates.ExecuteTemplate(w, "roles.html", map[string]interface{}{
		"Roles": roles,
	})
}