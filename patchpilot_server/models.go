package main

type Device struct {
	ID       int    `json:"id"`
	Hostname string `json:"hostname"`
	IP       string `json:"ip"`
	OS       string `json:"os"`
	LastSeen string `json:"last_seen"`
	Approved int    `json:"approved"`
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
	Timestamp string `json:"timestamp"`
}