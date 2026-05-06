package main

import "time"

type Device struct {
	ID        int
	Hostname  string
	IP        string
	OS        string
	LastSeen  string
	Approved  bool
}

type Action struct {
	ID        int       `json:"id"`
	Name      string    `json:"name"`
	DeviceID  int       `json:"device_id"`
	Status    string    `json:"status"`
	CreatedAt time.Time `json:"created_at"`
	UpdatedAt time.Time `json:"updated_at"`
}

type History struct {
	ID        int       `json:"id"`
	Action    string    `json:"action"`
	DeviceID  int       `json:"device_id"`
	CreatedAt time.Time `json:"created_at"`
}

type Audit struct {
	ID        int       `json:"id"`
	User      string    `json:"user"`
	Action    string    `json:"action"`
	Target    string    `json:"target"`
	Details   string    `json:"details"`
	CreatedAt time.Time `json:"created_at"`
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

type Settings struct {
	SiteName string `json:"site_name"`
}