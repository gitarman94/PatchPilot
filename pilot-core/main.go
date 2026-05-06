package main

import (
	"database/sql"
	"html/template"
	"log"
	"net/http"

	_ "github.com/mattn/go-sqlite3"
)

type App struct {
	DB        *sql.DB
	Templates *template.Template
}

func (a *App) renderTemplate(w http.ResponseWriter, name string, data interface{}) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	a.Templates.ExecuteTemplate(w, name, data)
}

func main() {
	db, err := sql.Open("sqlite3", "./kentro.db")
	if err != nil {
		log.Fatal(err)
	}

	initDB(db)

	tmpl := template.Must(template.ParseGlob("templates/*.html"))

	app := &App{
		DB:        db,
		Templates: tmpl,
	}

	mux := http.NewServeMux()

	mux.HandleFunc("/", app.login)
	mux.HandleFunc("/auth/login", app.login)
	mux.HandleFunc("/auth/logout", app.logout)

	mux.HandleFunc("/dashboard", app.requireAuth(app.dashboard))

	mux.HandleFunc("/devices_page", app.requireAuth(app.devicesPage))
	mux.HandleFunc("/device_detail/", app.requireAuth(app.deviceDetail))

	mux.HandleFunc("/actions_page", app.requireAuth(app.actionsPage))
	mux.HandleFunc("/history_page", app.requireAuth(app.historyPage))

	mux.HandleFunc("/users_groups_page", app.requireAuth(app.usersGroupsPage))
	mux.HandleFunc("/roles_page", app.requireAuth(app.rolesPage))
	mux.HandleFunc("/settings_page", app.requireAuth(app.settingsPage))

	mux.HandleFunc("/approve_device", app.requireAuth(app.approveDevice))
	mux.HandleFunc("/reject_device", app.requireAuth(app.rejectDevice))

	mux.HandleFunc("/submit_action", app.requireAuth(app.submitAction))
	mux.HandleFunc("/update_action_status", app.requireAuth(app.updateActionStatus))

	mux.HandleFunc("/create_user", app.requireAuth(app.createUser))
	mux.HandleFunc("/create_role", app.requireAuth(app.createRole))
	mux.HandleFunc("/update_setting", app.requireAuth(app.updateSetting))

	mux.HandleFunc("/api/devices", app.requireAuth(app.apiDevices))
	mux.HandleFunc("/api/actions", app.requireAuth(app.apiActions))
	mux.HandleFunc("/api/history", app.requireAuth(app.apiHistory))

	mux.Handle("/static/", http.StripPrefix("/static/", http.FileServer(http.Dir("static"))))

	log.Println("KentroCore running on :8080")
	log.Fatal(http.ListenAndServe(":8080", mux))
}

func (a *App) dashboard(w http.ResponseWriter, r *http.Request) {
	data := map[string]interface{}{}

	var totalDevices int64
	var approvedDevices int64
	var totalActions int64

	a.DB.QueryRow("SELECT COUNT(*) FROM devices").Scan(&totalDevices)
	a.DB.QueryRow("SELECT COUNT(*) FROM devices WHERE approved=1").Scan(&approvedDevices)
	a.DB.QueryRow("SELECT COUNT(*) FROM actions").Scan(&totalActions)

	data["total_devices"] = totalDevices
	data["approved_devices"] = approvedDevices
	data["total_actions"] = totalActions
	data["pending_devices"] = totalDevices - approvedDevices

	a.renderTemplate(w, "dashboard.html", data)
}