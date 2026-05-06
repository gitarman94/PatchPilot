package main

import "net/http"

func (a *App) usersGroupsPage(w http.ResponseWriter, r *http.Request) {
	users, err := a.getAllUsers()
	if err != nil {
		http.Error(w, "Failed to load users", http.StatusInternalServerError)
		return
	}

	groups, err := a.getAllGroups()
	if err != nil {
		http.Error(w, "Failed to load groups", http.StatusInternalServerError)
		return
	}

	a.Templates.ExecuteTemplate(w, "users_groups.html", map[string]interface{}{
		"Users":  users,
		"Groups": groups,
	})
}

func (a *App) getAllUsers() ([]User, error) {
	rows, err := a.DB.Query("SELECT id, username, password_hash, role_id FROM users ORDER BY id DESC")
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var users []User
	for rows.Next() {
		var u User
		err := rows.Scan(&u.ID, &u.Username, &u.PasswordHash, &u.RoleID)
		if err != nil {
			continue
		}
		users = append(users, u)
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	return users, nil
}

func (a *App) getAllGroups() ([]Group, error) {
	rows, err := a.DB.Query("SELECT id, name FROM groups ORDER BY id DESC")
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	var groups []Group
	for rows.Next() {
		var g Group
		err := rows.Scan(&g.ID, &g.Name)
		if err != nil {
			continue
		}
		groups = append(groups, g)
	}

	if err := rows.Err(); err != nil {
		return nil, err
	}

	return groups, nil
}