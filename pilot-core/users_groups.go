package main

import (
	"net/http"
)

// Users & Groups page handler
func usersGroupsHandler(w http.ResponseWriter, r *http.Request) {
	users, err := getAllUsers()
	if err != nil {
		http.Error(w, "Failed to load users", http.StatusInternalServerError)
		return
	}

	groups, err := getAllGroups()
	if err != nil {
		http.Error(w, "Failed to load groups", http.StatusInternalServerError)
		return
	}

	renderTemplate(w, "users_groups.html", map[string]interface{}{
		"Users":  users,
		"Groups": groups,
	})
}

// DB functions (use structs from models.go)

func getAllUsers() ([]User, error) {
	rows, err := db.Query("SELECT id, username, password_hash, role_id FROM users")
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var users []User
	for rows.Next() {
		var u User
		err := rows.Scan(&u.ID, &u.Username, &u.PasswordHash, &u.RoleID)
		if err != nil {
			return nil, err
		}
		users = append(users, u)
	}
	return users, nil
}

func getAllGroups() ([]Group, error) {
	rows, err := db.Query("SELECT id, name FROM groups")
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var groups []Group
	for rows.Next() {
		var g Group
		err := rows.Scan(&g.ID, &g.Name)
		if err != nil {
			return nil, err
		}
		groups = append(groups, g)
	}
	return groups, nil
}