package main

import "time"

type Device struct {
	ID              int     `json:"id"`
	AgentID         string  `json:"agent_id"`
	Hostname        string  `json:"hostname"`
	FQDN            string  `json:"fqdn"`
	IP              string  `json:"ip"`
	OS              string  `json:"os"`
	Architecture    string  `json:"architecture"`
	DeviceType      string  `json:"device_type"`
	DeviceModel     string  `json:"device_model"`
	CPUModel        string  `json:"cpu_model"`
	CPUUsage        float64 `json:"cpu_usage"`
	RAMTotal        int64   `json:"ram_total"`
	RAMUsed         int64   `json:"ram_used"`
	RAMUsagePercent float64 `json:"ram_usage_percent"`
	DiskTotal       int64   `json:"disk_total"`
	DiskUsed        int64   `json:"disk_used"`
	DiskFree        int64   `json:"disk_free"`
	DiskFreeHuman   string  `json:"disk_free_human"`
	LastSeen        string  `json:"last_seen"`
	Approved        bool    `json:"approved"`
}

type Action struct {
	ID        int    `json:"id"`
	Name      string `json:"name"`
	DeviceID  int    `json:"device_id"`
	Status    string `json:"status"`
	CreatedAt string `json:"created_at"`
	UpdatedAt string `json:"updated_at"`
}

type History struct {
	ID        int    `json:"id"`
	Action    string `json:"action"`
	DeviceID  int    `json:"device_id"`
	CreatedAt string `json:"created_at"`
}

type Audit struct {
	ID        int    `json:"id"`
	User      string `json:"user"`
	Action    string `json:"action"`
	Target    string `json:"target"`
	Details   string `json:"details"`
	CreatedAt string `json:"created_at"`
}

type User struct {
	ID           int    `json:"id"`
	Username     string `json:"username"`
	PasswordHash string `json:"-"`
	RoleID       int    `json:"role_id"`
}

type Group struct {
	ID   int    `json:"id"`
	Name string `json:"name"`
}

type Role struct {
	ID          int    `json:"id"`
	Name        string `json:"name"`
	Description string `json:"description"`
}

type Setting struct {
	Key   string `json:"key"`
	Value string `json:"value"`
}

type AgentUpdate struct {
	ID           int    `json:"id"`
	Version      string `json:"version"`
	Platform     string `json:"platform"`
	Arch         string `json:"arch"`
	Filename     string `json:"filename"`
	OriginalName string `json:"original_name"`
	SHA256       string `json:"sha256"`
	SizeBytes    int64  `json:"size_bytes"`
	Active       bool   `json:"active"`
	UploadedAt   string `json:"uploaded_at"`
}

type DashboardData struct {
	TotalDevices   int
	ApprovedDevices int
	PendingDevices int
	OnlineDevices  int
	TotalActions   int
	Devices        []Device
	Actions        []Action
}

type DevicesPageData struct {
	Devices        []Device
	TotalDevices   int
	ApprovedDevices int
	PendingDevices int
	OnlineDevices  int
}

type DeviceDetailPageData struct {
	Device *Device
}

type AgentCheckinRequest struct {
	AgentID         string  `json:"agent_id"`
	Hostname        string  `json:"hostname"`
	FQDN            string  `json:"fqdn"`
	IP              string  `json:"ip"`
	OS              string  `json:"os"`
	Architecture    string  `json:"architecture"`
	DeviceType      string  `json:"device_type"`
	DeviceModel     string  `json:"device_model"`
	CPUModel        string  `json:"cpu_model"`
	CPUUsage        float64 `json:"cpu_usage"`
	RAMTotal        int64   `json:"ram_total"`
	RAMUsed         int64   `json:"ram_used"`
	RAMUsagePercent float64 `json:"ram_usage_percent"`
	DiskTotal       int64   `json:"disk_total"`
	DiskUsed        int64   `json:"disk_used"`
	DiskFree        int64   `json:"disk_free"`
	DiskFreeHuman   string  `json:"disk_free_human"`
	Version         string  `json:"version"`
}

type AgentCheckinResponse struct {
	Status   string `json:"status"`
	DeviceID int    `json:"device_id"`
	Approved bool   `json:"approved"`
}

type AgentUpdateResponse struct {
	Version string `json:"version"`
	URL     string `json:"url"`
}