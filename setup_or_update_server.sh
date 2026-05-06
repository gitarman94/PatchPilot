#!/usr/bin/env bash
set -euo pipefail

APP_DIR="/opt/kentro"
SERVICE_NAME="kentrocore.service"
SYSTEMD_DIR="/etc/systemd/system"

echo "Starting Kentro setup..."

# --- OS check ---
if [[ -f /etc/os-release ]]; then
    . /etc/os-release
    case "$ID" in
        debian|ubuntu|linuxmint|pop|raspbian) ;;
        *) echo "Only Debian-based systems supported."; exit 1 ;;
    esac
else
    echo "Cannot determine OS."; exit 1
fi

# --- stop old service ---
systemctl stop "${SERVICE_NAME}" 2>/dev/null || true
systemctl disable "${SERVICE_NAME}" 2>/dev/null || true

# --- install deps ---
apt-get update -qq
apt-get install -y -qq curl build-essential sqlite3 libsqlite3-dev

# --- install Go if missing ---
if ! command -v go >/dev/null 2>&1; then
    curl -LO https://go.dev/dl/go1.22.5.linux-amd64.tar.gz
    rm -rf /usr/local/go
    tar -C /usr/local -xzf go1.22.5.linux-amd64.tar.gz
    export PATH=$PATH:/usr/local/go/bin
fi

export PATH=$PATH:/usr/local/go/bin

# --- create user ---
if ! id -u kentro >/dev/null 2>&1; then
    useradd -r -m -d /home/kentro -s /usr/sbin/nologin kentro || true
fi

# --- setup dirs ---
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR"
cd "$APP_DIR"

# --- create go module ---
cat > go.mod <<EOF
module kentro

go 1.22

require github.com/mattn/go-sqlite3 v1.14.22
EOF

# --- main.go ---
cat > main.go <<'EOF'
package main

import (
	"database/sql"
	"html/template"
	"log"
	"net/http"
	"os"

	_ "github.com/mattn/go-sqlite3"
)

type App struct {
	DB        *sql.DB
	Templates *template.Template
}

type DashboardData struct {
	TotalDevices    int
	ApprovedDevices int
	PendingDevices  int
	TotalActions    int
}

func main() {
	dbPath := getEnv("DATABASE_PATH", "./kentro.db")
	addr := getEnv("SERVER_ADDRESS", "0.0.0.0")
	port := getEnv("SERVER_PORT", "8080")

	db, err := sql.Open("sqlite3", dbPath)
	if err != nil {
		log.Fatal(err)
	}

	if err := db.Ping(); err != nil {
		log.Fatal(err)
	}

	initDB(db)

	tmpl := template.Must(template.ParseGlob("templates/*.html"))

	app := &App{
		DB:        db,
		Templates: tmpl,
	}

	mux := http.NewServeMux()

	mux.HandleFunc("/", app.redirect)
	mux.HandleFunc("/dashboard", app.dashboard)

	mux.Handle("/static/", http.StripPrefix("/static/", http.FileServer(http.Dir("static"))))

	log.Printf("KentroCore running on %s:%s\n", addr, port)
	log.Fatal(http.ListenAndServe(addr+":"+port, mux))
}

func getEnv(key, fallback string) string {
	if v := os.Getenv(key); v != "" {
		return v
	}
	return fallback
}

func initDB(db *sql.DB) {
	db.Exec(`CREATE TABLE IF NOT EXISTS devices (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		name TEXT,
		approved INTEGER DEFAULT 0
	)`)

	db.Exec(`CREATE TABLE IF NOT EXISTS actions (
		id INTEGER PRIMARY KEY AUTOINCREMENT
	)`)
}

func (a *App) redirect(w http.ResponseWriter, r *http.Request) {
	http.Redirect(w, r, "/dashboard", http.StatusFound)
}

func (a *App) dashboard(w http.ResponseWriter, r *http.Request) {
	data := DashboardData{}

	a.DB.QueryRow("SELECT COUNT(*) FROM devices").Scan(&data.TotalDevices)
	a.DB.QueryRow("SELECT COUNT(*) FROM devices WHERE approved = 1").Scan(&data.ApprovedDevices)
	a.DB.QueryRow("SELECT COUNT(*) FROM actions").Scan(&data.TotalActions)

	data.PendingDevices = data.TotalDevices - data.ApprovedDevices

	w.Header().Set("Content-Type", "text/html; charset=utf-8")
	a.Templates.ExecuteTemplate(w, "dashboard.html", data)
}
EOF

# --- templates ---
mkdir -p templates

cat > templates/dashboard.html <<'EOF'
{{ define "dashboard.html" }}
<!DOCTYPE html>
<html>
<head>
<title>Kentro Dashboard</title>
<link rel="stylesheet" href="/static/styles.css">
</head>
<body>

{{ template "navbar.html" . }}

<h1>Kentro Dashboard</h1>

<div>Total Devices: {{.TotalDevices}}</div>
<div>Approved: {{.ApprovedDevices}}</div>
<div>Pending: {{.PendingDevices}}</div>
<div>Total Actions: {{.TotalActions}}</div>

</body>
</html>
{{ end }}
EOF

cat > templates/navbar.html <<'EOF'
{{ define "navbar.html" }}
<nav>
<a href="/dashboard">Dashboard</a>
</nav>
{{ end }}
EOF

# --- static ---
mkdir -p static

echo "body { font-family: sans-serif; }" > static/styles.css

# --- env ---
cat > .env <<EOF
DATABASE_PATH=/opt/kentro/kentro.db
SERVER_ADDRESS=0.0.0.0
SERVER_PORT=8080
EOF

# --- build ---
go mod tidy
go build -o kentrocore

# --- permissions ---
chown -R kentro:kentro "$APP_DIR"
chmod +x kentrocore

# --- systemd ---
cat > "${SYSTEMD_DIR}/${SERVICE_NAME}" <<EOF
[Unit]
Description=Kentro Core Server
After=network.target

[Service]
User=kentro
Group=kentro
WorkingDirectory=${APP_DIR}
EnvironmentFile=${APP_DIR}/.env
ExecStart=${APP_DIR}/kentrocore
Restart=on-failure

[Install]
WantedBy=multi-user.target
EOF

systemctl daemon-reload
systemctl enable --now "${SERVICE_NAME}"

IP=$(hostname -I | awk '{print $1}')
echo "Kentro running at http://${IP}:8080"