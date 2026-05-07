package main

import (
	"crypto/sha256"
	"database/sql"
	"encoding/hex"
	"fmt"
	htmltemplate "html/template"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

type updateCheckResponse struct {
	UpdateAvailable bool   `json:"update_available"`
	Version         string `json:"version,omitempty"`
	Platform        string `json:"platform,omitempty"`
	Arch            string `json:"arch,omitempty"`
	Filename        string `json:"filename,omitempty"`
	DownloadURL     string `json:"download_url,omitempty"`
	SHA256          string `json:"sha256,omitempty"`
	SizeBytes       int64  `json:"size_bytes,omitempty"`
}

func (a *App) agentUpdatesPage(w http.ResponseWriter, r *http.Request) {
	rows, err := a.DB.Query(`
		SELECT id, version, platform, arch, filename, IFNULL(original_name, ''), sha256, size_bytes, active, IFNULL(uploaded_at, '')
		FROM agent_updates
		ORDER BY id DESC
	`)
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}
	defer rows.Close()

	var updates []AgentUpdate
	for rows.Next() {
		var u AgentUpdate
		if err := rows.Scan(&u.ID, &u.Version, &u.Platform, &u.Arch, &u.Filename, &u.OriginalName, &u.SHA256, &u.SizeBytes, &u.Active, &u.UploadedAt); err != nil {
			continue
		}
		updates = append(updates, u)
	}

	w.Header().Set("Content-Type", "text/html; charset=utf-8")

	var b strings.Builder
	b.WriteString(`<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Agent Updates - CommandPilot</title><link rel="stylesheet" href="/static/styles.css"></head><body>`)
	b.WriteString(`<main class="container">`)
	b.WriteString(`<section class="page-header"><div><h1>Agent Updates</h1><p>Upload release ZIPs and activate the version pushed to clients.</p></div></section>`)
	b.WriteString(`<section class="dashboard-grid">`)

	b.WriteString(`<div class="panel"><div class="panel-header"><h2>Upload Update Package</h2><div class="panel-subtitle">Upload a ZIP built from the desired release contents</div></div><div style="padding:24px;">`)
	b.WriteString(`<form method="POST" action="/upload_agent_update" enctype="multipart/form-data">`)
	b.WriteString(`<div class="form-group"><label for="version">Version</label><input id="version" name="version" type="text" placeholder="1.2.3" required></div>`)
	b.WriteString(`<div class="form-group"><label for="platform">Platform</label><input id="platform" name="platform" type="text" placeholder="linux" value="linux" required></div>`)
	b.WriteString(`<div class="form-group"><label for="arch">Arch</label><input id="arch" name="arch" type="text" placeholder="amd64" value="amd64" required></div>`)
	b.WriteString(`<div class="form-group"><label for="archive">ZIP File</label><input id="archive" name="archive" type="file" accept=".zip,application/zip" required></div>`)
	b.WriteString(`<div class="form-group"><label for="notes">Notes</label><input id="notes" name="notes" type="text" placeholder="Optional release notes"></div>`)
	b.WriteString(`<label style="display:block;margin-bottom:16px;"><input type="checkbox" name="active" value="1"> Activate after upload</label>`)
	b.WriteString(`<button type="submit" class="btn btn-primary">Upload Package</button>`)
	b.WriteString(`</form></div></div>`)

	b.WriteString(`<div class="panel"><div class="panel-header"><h2>Available Packages</h2><div class="panel-subtitle">Latest active package is what clients will receive</div></div><div style="padding:24px;">`)
	b.WriteString(`<table class="data-table"><thead><tr><th>ID</th><th>Version</th><th>Platform</th><th>Arch</th><th>File</th><th>SHA256</th><th>Status</th><th>Action</th></tr></thead><tbody>`)

	if len(updates) == 0 {
		b.WriteString(`<tr><td colspan="8" class="empty-state">No agent update packages have been uploaded.</td></tr>`)
	} else {
		for _, u := range updates {
			status := "inactive"
			if u.Active {
				status = "active"
			}
			fileLink := "/updates/" + htmltemplate.HTMLEscapeString(u.Filename)
			b.WriteString("<tr>")
			b.WriteString(fmt.Sprintf("<td>%d</td>", u.ID))
			b.WriteString("<td>" + htmltemplate.HTMLEscapeString(u.Version) + "</td>")
			b.WriteString("<td>" + htmltemplate.HTMLEscapeString(u.Platform) + "</td>")
			b.WriteString("<td>" + htmltemplate.HTMLEscapeString(u.Arch) + "</td>")
			b.WriteString(`<td><a href="` + fileLink + `">` + htmltemplate.HTMLEscapeString(u.Filename) + `</a></td>`)
			b.WriteString("<td><code>" + htmltemplate.HTMLEscapeString(u.SHA256) + "</code></td>")
			b.WriteString("<td>" + htmltemplate.HTMLEscapeString(status) + "</td>")
			b.WriteString(`<td>`)
			if !u.Active {
				b.WriteString(`<form method="POST" action="/activate_agent_update" style="margin:0;"><input type="hidden" name="id" value="` + strconv.Itoa(u.ID) + `"><button type="submit" class="btn btn-primary">Activate</button></form>`)
			} else {
				b.WriteString(`Active`)
			}
			b.WriteString(`</td>`)
			b.WriteString("</tr>")
		}
	}

	b.WriteString(`</tbody></table></div></div>`)
	b.WriteString(`</section></main></body></html>`)
	_, _ = w.Write([]byte(b.String()))
}

func (a *App) uploadAgentUpdate(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}

	if err := r.ParseMultipartForm(128 << 20); err != nil {
		http.Error(w, "invalid multipart form", http.StatusBadRequest)
		return
	}

	version := strings.TrimSpace(r.FormValue("version"))
	platform := normalizeUpdateToken(r.FormValue("platform"))
	arch := normalizeUpdateToken(r.FormValue("arch"))
	notes := strings.TrimSpace(r.FormValue("notes"))

	if version == "" {
		http.Error(w, "version is required", http.StatusBadRequest)
		return
	}
	if platform == "" {
		platform = "linux"
	}
	if arch == "" {
		arch = "amd64"
	}

	file, header, err := r.FormFile("archive")
	if err != nil {
		http.Error(w, "archive is required", http.StatusBadRequest)
		return
	}
	defer file.Close()

	if !strings.HasSuffix(strings.ToLower(header.Filename), ".zip") {
		http.Error(w, "archive must be a zip file", http.StatusBadRequest)
		return
	}

	if err := os.MkdirAll("updates", 0755); err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	finalName := fmt.Sprintf("%s-%s-%s-%d.zip", sanitizeUpdateSegment(platform), sanitizeUpdateSegment(arch), sanitizeUpdateSegment(version), time.Now().UnixNano())
	finalPath := filepath.Join("updates", finalName)

	tmpFile, err := os.CreateTemp("updates", ".upload-*.tmp")
	if err != nil {
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	hasher := sha256.New()
	written, copyErr := io.Copy(io.MultiWriter(tmpFile, hasher), file)
	closeErr := tmpFile.Close()
	if copyErr != nil {
		_ = os.Remove(tmpFile.Name())
		http.Error(w, copyErr.Error(), http.StatusInternalServerError)
		return
	}
	if closeErr != nil {
		_ = os.Remove(tmpFile.Name())
		http.Error(w, closeErr.Error(), http.StatusInternalServerError)
		return
	}

	if err := os.Rename(tmpFile.Name(), finalPath); err != nil {
		_ = os.Remove(tmpFile.Name())
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	sum := hex.EncodeToString(hasher.Sum(nil))

	res, err := a.DB.Exec(`
		INSERT INTO agent_updates (
			version, platform, arch, filename, original_name, sha256, size_bytes, active, uploaded_at
		) VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?)
	`, version, platform, arch, finalName, header.Filename, sum, written, time.Now().Format(time.RFC3339))
	if err != nil {
		_ = os.Remove(finalPath)
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	updateID, _ := res.LastInsertId()

	if r.FormValue("active") == "1" {
		if err := a.activateAgentUpdateByID(int(updateID)); err != nil {
			http.Error(w, err.Error(), http.StatusInternalServerError)
			return
		}
	}

	_ = notes

	http.Redirect(w, r, "/agent_updates_page", http.StatusSeeOther)
}

func (a *App) activateAgentUpdate(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "method not allowed", http.StatusMethodNotAllowed)
		return
	}

	id, err := strconv.Atoi(r.FormValue("id"))
	if err != nil {
		http.Error(w, "invalid id", http.StatusBadRequest)
		return
	}

	if err := a.activateAgentUpdateByID(id); err != nil {
		if err == sql.ErrNoRows {
			http.Error(w, "update not found", http.StatusNotFound)
			return
		}
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	http.Redirect(w, r, "/agent_updates_page", http.StatusSeeOther)
}

func (a *App) activateAgentUpdateByID(id int) error {
	tx, err := a.DB.Begin()
	if err != nil {
		return err
	}
	defer func() {
		if err != nil {
			_ = tx.Rollback()
		}
	}()

	var platform string
	var arch string

	err = tx.QueryRow(`SELECT platform, arch FROM agent_updates WHERE id = ?`, id).Scan(&platform, &arch)
	if err != nil {
		_ = tx.Rollback()
		return err
	}

	if _, err = tx.Exec(`UPDATE agent_updates SET active = 0 WHERE platform = ? AND arch = ?`, platform, arch); err != nil {
		_ = tx.Rollback()
		return err
	}

	if _, err = tx.Exec(`UPDATE agent_updates SET active = 1 WHERE id = ?`, id); err != nil {
		_ = tx.Rollback()
		return err
	}

	return tx.Commit()
}

func (a *App) apiAgentUpdateCheck(w http.ResponseWriter, r *http.Request) {
	platform := strings.TrimSpace(r.URL.Query().Get("platform"))
	arch := strings.TrimSpace(r.URL.Query().Get("arch"))
	clientVersion := strings.TrimSpace(r.URL.Query().Get("version"))

	query := `
		SELECT id, version, platform, arch, filename, IFNULL(original_name, ''), sha256, size_bytes, active, IFNULL(uploaded_at, '')
		FROM agent_updates
		WHERE active = 1
	`
	args := []any{}

	if platform != "" {
		query += " AND platform = ?"
		args = append(args, platform)
	}
	if arch != "" {
		query += " AND arch = ?"
		args = append(args, arch)
	}

	query += " ORDER BY uploaded_at DESC, id DESC LIMIT 1"

	var update AgentUpdate
	err := a.DB.QueryRow(query, args...).Scan(
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
		if err == sql.ErrNoRows {
			writeJSON(w, updateCheckResponse{UpdateAvailable: false})
			return
		}
		http.Error(w, err.Error(), http.StatusInternalServerError)
		return
	}

	if clientVersion != "" && compareVersions(update.Version, clientVersion) <= 0 {
		writeJSON(w, updateCheckResponse{UpdateAvailable: false})
		return
	}

	writeJSON(w, updateCheckResponse{
		UpdateAvailable: true,
		Version:         update.Version,
		Platform:        update.Platform,
		Arch:            update.Arch,
		Filename:        update.Filename,
		DownloadURL:     "/updates/" + update.Filename,
		SHA256:          update.SHA256,
		SizeBytes:       update.SizeBytes,
	})
}

func compareVersions(a, b string) int {
	ap := parseVersionParts(a)
	bp := parseVersionParts(b)

	maxLen := len(ap)
	if len(bp) > maxLen {
		maxLen = len(bp)
	}

	for i := 0; i < maxLen; i++ {
		var av, bv int
		if i < len(ap) {
			av = ap[i]
		}
		if i < len(bp) {
			bv = bp[i]
		}
		if av < bv {
			return -1
		}
		if av > bv {
			return 1
		}
	}

	return 0
}

func parseVersionParts(v string) []int {
	v = strings.TrimSpace(strings.ToLower(v))
	v = strings.TrimPrefix(v, "v")
	v = strings.SplitN(v, "+", 2)[0]

	fields := strings.FieldsFunc(v, func(r rune) bool {
		return r == '.' || r == '-' || r == '_'
	})

	out := make([]int, 0, len(fields))
	for _, field := range fields {
		n := 0
		seen := false
		for _, r := range field {
			if r < '0' || r > '9' {
				break
			}
			n = n*10 + int(r-'0')
			seen = true
		}
		if !seen {
			out = append(out, 0)
			continue
		}
		out = append(out, n)
	}

	return out
}

func normalizeUpdateToken(s string) string {
	return strings.ToLower(strings.TrimSpace(s))
}

func sanitizeUpdateSegment(s string) string {
	s = strings.ToLower(strings.TrimSpace(s))
	if s == "" {
		return "unknown"
	}

	var b strings.Builder
	lastDash := false

	for _, r := range s {
		if (r >= 'a' && r <= 'z') || (r >= '0' && r <= '9') {
			b.WriteRune(r)
			lastDash = false
			continue
		}

		if !lastDash {
			b.WriteRune('-')
			lastDash = true
		}
	}

	out := strings.Trim(b.String(), "-")
	if out == "" {
		return "unknown"
	}

	return out
}