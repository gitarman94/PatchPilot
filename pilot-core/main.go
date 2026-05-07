package main

import (
	"database/sql"
	"encoding/json"
	"html/template"
	"log"
	"net/http"
	"os"
	"strings"
)

type App struct {
	DB        *sql.DB
	Templates *template.Template
}

func (a *App) renderTemplate(w http.ResponseWriter, name string, data interface{}) {
	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	if err := a.Templates.ExecuteTemplate(w, name, data); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
	}
}

func writeJSON(w http.ResponseWriter, v any) {
	w.Header().Set("Content-Type", "application/json; charset=utf-8")
	enc := json.NewEncoder(w)
	enc.SetEscapeHTML(true)
	if err := enc.Encode(v); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
	}
}

func main() {
	if err := os.MkdirAll("updates", 0755); err != nil {
		log.Fatal(err)
	}

	db, err := sql.Open("sqlite3", "./commandpilot.db")
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
	mux.HandleFunc("/change_password", app.requireAuth(app.changePassword))
	mux.HandleFunc("/approve_device", app.requireAuth(app.approveDevice))
	mux.HandleFunc("/reject_device", app.requireAuth(app.rejectDevice))
	mux.HandleFunc("/submit_action", app.requireAuth(app.submitAction))
	mux.HandleFunc("/update_action_status", app.requireAuth(app.updateActionStatus))
	mux.HandleFunc("/create_user", app.requireAuth(app.createUser))
	mux.HandleFunc("/create_role", app.requireAuth(app.createRole))
	mux.HandleFunc("/update_setting", app.requireAuth(app.updateSetting))
	mux.HandleFunc("/agent_updates_page", app.requireAuth(app.agentUpdatesPage))
	mux.HandleFunc("/upload_agent_update", app.requireAuth(app.uploadAgentUpdate))
	mux.HandleFunc("/activate_agent_update", app.requireAuth(app.activateAgentUpdate))

	mux.HandleFunc("/api/agent/checkin", app.agentCheckinHandler)
	mux.HandleFunc("/api/agent/update/check", app.apiAgentUpdateCheck)
	mux.HandleFunc("/api/agent/update", app.apiAgentUpdateCheck)

	mux.HandleFunc("/api/devices", app.requireAuth(app.apiDevices))
	mux.HandleFunc("/api/actions", app.requireAuth(app.apiActions))
	mux.HandleFunc("/api/history", app.requireAuth(app.apiHistory))

	mux.Handle("/updates/", http.StripPrefix("/updates/", http.FileServer(http.Dir("updates"))))
	mux.Handle("/static/", http.StripPrefix("/static/", http.FileServer(http.Dir("static"))))

	log.Println("CommandPilot running on :8080")
	log.Fatal(http.ListenAndServe(":8080", mux))
}

func (a *App) dashboard(w http.ResponseWriter, r *http.Request) {
	data := DashboardData{}

	data.TotalDevices = queryCount(a.DB, `SELECT COUNT(*) FROM devices`)
	data.ApprovedDevices = queryCount(a.DB, `SELECT COUNT(*) FROM devices WHERE approved = 1`)
	data.PendingDevices = queryCount(a.DB, `SELECT COUNT(*) FROM devices WHERE approved = 0`)
	data.OnlineDevices = queryCount(a.DB, `
SELECT COUNT(*)
FROM devices
WHERE last_seen IS NOT NULL
  AND last_seen <> ''
  AND datetime(last_seen) >= datetime('now', '-5 minutes')
`)
	data.TotalActions = queryCount(a.DB, `SELECT COUNT(*) FROM actions`)

	devices, err := loadRecentDevices(a.DB, 10)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	data.Devices = devices

	actions, err := loadRecentActions(a.DB, 10)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	data.Actions = actions

	a.renderTemplate(w, "dashboard.html", data)
}

func queryCount(db *sql.DB, query string, args ...any) int {
	var count int
	if err := db.QueryRow(query, args...).Scan(&count); err != nil {
		return 0
	}
	return count
}

func (a *App) apiAgentUpdateCheck(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}

	currentVersion := strings.TrimSpace(r.URL.Query().Get("version"))
	reqPlatform := strings.TrimSpace(r.URL.Query().Get("platform"))
	reqArch := strings.TrimSpace(r.URL.Query().Get("arch"))

	var update AgentUpdate
	err := a.DB.QueryRow(`
SELECT id, version, platform, arch, filename, original_name, sha256, size_bytes, active, IFNULL(uploaded_at, '')
FROM agent_updates
WHERE active = 1
ORDER BY id DESC
LIMIT 1
`).Scan(
		&update.ID,
		&update.Version,
		&update.Platform,
		&update.Arch,
		&update.Filename,
		&update.OriginalName,
		&update.SHA256,
		&update.SizeBytes,
		&update.Active,
		&update.UploadedAt,
	)

	if err != nil {
		w.WriteHeader(http.StatusNoContent)
		return
	}

	if reqPlatform != "" && !strings.EqualFold(reqPlatform, update.Platform) {
		w.WriteHeader(http.StatusNoContent)
		return
	}

	if reqArch != "" && !strings.EqualFold(reqArch, update.Arch) {
		w.WriteHeader(http.StatusNoContent)
		return
	}

	if currentVersion == "" || currentVersion == update.Version {
		w.WriteHeader(http.StatusNoContent)
		return
	}

	writeJSON(w, AgentUpdateResponse{
		Version: update.Version,
		URL:     "/updates/" + update.Filename,
	})
}