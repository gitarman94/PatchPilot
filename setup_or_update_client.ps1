# setup_or_update_client.ps1
Param (
    [string]$ServerUrl
)

$ErrorActionPreference = "Stop"

# === Config ===
$GitHubUser = "gitarman94"
$GitHubRepo = "PatchPilot"
$Branch = "main"

$RawBase = "https://raw.githubusercontent.com/$GitHubUser/$GitHubRepo/$Branch/windows-client"
$InstallDir = "C:\PatchPilot_Client"

# Files to update/check
$FilesToUpdate = @(
    "patchpilot_client.exe",
    "patchpilot_updater.exe",
    "config.json",
    "patchpilot_client.ps1",
    "patchpilot_ping.ps1"
)

# Helper to compute file hash
function Get-FileHashString($path) {
    if (Test-Path $path) {
        return (Get-FileHash -Algorithm SHA256 -Path $path).Hash
    }
    return ""
}

# Download file helper
function Download-File($url, $dest) {
    Invoke-WebRequest -Uri $url -OutFile $dest -UseBasicParsing
}

# Update existing files if changed
function Update-Files {
    Write-Host "🔍 Checking for client updates via SHA256 hash..."

    $updated = $false

    foreach ($file in $FilesToUpdate) {
        $localPath = Join-Path $InstallDir $file
        $tempRemote = Join-Path $env:TEMP "$file.remote"

        $remoteUrl = "$RawBase/$file"
        Write-Host "📁 Checking: $file"
        Download-File $remoteUrl $tempRemote

        $remoteHash = Get-FileHashString $tempRemote
        $localHash = Get-FileHashString $localPath

        if ($remoteHash -ne $localHash) {
            Write-Host "⬆️  $file is outdated. Updating..."
            Copy-Item -Path $tempRemote -Destination $localPath -Force
            $updated = $true
        } else {
            Write-Host "✅ $file is up to date."
        }

        Remove-Item $tempRemote -Force
    }

    if ($updated) {
        Write-Host "🔁 Restarting client scheduled tasks to apply updates..."

        # Restart scheduled tasks
        foreach ($taskName in @("PatchPilot_Client", "PatchPilot_Ping")) {
            if (Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue) {
                Restart-ScheduledTask -TaskName $taskName
            }
        }

        Write-Host "✅ Client update complete."
    } else {
        Write-Host "🚀 No client updates detected. Everything is current."
    }
}

# Full install
function Install-Client {
    Write-Host "[*] Installing dependencies..."

    # Install Chocolatey if missing (optional - to get jq, git, curl)
    if (-not (Get-Command choco.exe -ErrorAction SilentlyContinue)) {
        Write-Host "Installing Chocolatey..."
        Set-ExecutionPolicy Bypass -Scope Process -Force
        iex ((New-Object System.Net.WebClient).DownloadString('https://chocolatey.org/install.ps1'))
    }

    # Install git, curl, jq if missing
    foreach ($pkg in @("git", "curl", "jq")) {
        if (-not (Get-Command $pkg -ErrorAction SilentlyContinue)) {
            Write-Host "Installing $pkg..."
            choco install $pkg -y
        }
    }

    Write-Host "[*] Creating install directory..."
    if (Test-Path $InstallDir) {
        Remove-Item -Recurse -Force $InstallDir
    }
    New-Item -Path $InstallDir -ItemType Directory | Out-Null

    # Download binaries and files from GitHub raw
    foreach ($file in $FilesToUpdate) {
        $url = "$RawBase/$file"
        $dest = Join-Path $InstallDir $file
        Write-Host "Downloading $file..."
        Download-File $url $dest
    }

    # Generate client_id.txt if missing
    $clientIdPath = Join-Path $InstallDir "client_id.txt"
    if (-not (Test-Path $clientIdPath)) {
        [guid]::NewGuid().ToString() | Out-File -Encoding ASCII $clientIdPath
    }

    # Server URL prompt if not passed as param
    if (-not $ServerUrl) {
        $ServerUrl = Read-Host "Enter the patch server URL (e.g., http://192.168.1.100:8080)"
    }
    # Strip protocol if present
    $ServerUrl = $ServerUrl -replace '^https?://', ''

    # Save server URL
    $serverUrlPath = Join-Path $InstallDir "server_url.txt"
    $ServerUrl | Out-File -Encoding ASCII $serverUrlPath

    # Setup Scheduled Tasks for client and ping scripts
    Write-Host "[*] Creating scheduled tasks..."

    $clientTaskName = "PatchPilot_Client"
    $pingTaskName = "PatchPilot_Ping"

    # Remove existing tasks if any
    foreach ($taskName in @($clientTaskName, $pingTaskName)) {
        if (Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue) {
            Unregister-ScheduledTask -TaskName $taskName -Confirm:$false
        }
    }

    # Create client task - runs every 10 minutes (adjust as needed)
    $clientAction = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-File `"$InstallDir\patchpilot_client.ps1`""
    $clientTrigger = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes(1) -RepetitionInterval (New-TimeSpan -Minutes 10) -RepetitionDuration ([TimeSpan]::MaxValue)
    Register-ScheduledTask -TaskName $clientTaskName -Action $clientAction -Trigger $clientTrigger -Description "PatchPilot Client" -User "SYSTEM" -RunLevel Highest

    # Create ping task - runs every 5 minutes (adjust as needed)
    $pingAction = New-ScheduledTaskAction -Execute "powershell.exe" -Argument "-File `"$InstallDir\patchpilot_ping.ps1`""
    $pingTrigger = New-ScheduledTaskTrigger -Once -At (Get-Date).AddMinutes(1) -RepetitionInterval (New-TimeSpan -Minutes 5) -RepetitionDuration ([TimeSpan]::MaxValue)
    Register-ScheduledTask -TaskName $pingTaskName -Action $pingAction -Trigger $pingTrigger -Description "PatchPilot Ping" -User "SYSTEM" -RunLevel Highest

    Write-Host "[✓] Installation complete. Client is active."
}

# === Main ===

if (Test-Path "$InstallDir\patchpilot_client.exe") {
    Write-Host "[*] Detected existing client installation. Running update..."
    Update-Files
} else {
    Write-Host "[*] No client installation detected. Running full install..."
    Install-Client
}
