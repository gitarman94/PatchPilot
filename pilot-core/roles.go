package main

import (
	"net/http"
)

// Roles page handler
func rolesHandler(w http.ResponseWriter, r *http.Request) {
	roles, err := getAllRoles()
	if err != nil {
		http.Error(w, "Failed to load roles", http.StatusInternalServerError)
		return
	}

	renderTemplate(w, "roles.html", map[string]interface{}{
		"Roles": roles,
	})
}

// DB function (uses Role from models.go)
func getAllRoles() ([]Role, error) {
	rows, err := db.Query("SELECT id, name, description FROM roles")
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var roles []Role
	for rows.Next() {
		var r Role
		err := rows.Scan(&r.ID, &r.Name, &r.Description)
		if err != nil {
			return nil, err
		}
		roles = append(roles, r)
	}
	return roles, nil
}