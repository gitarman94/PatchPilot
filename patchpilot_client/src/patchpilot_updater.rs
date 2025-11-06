#!/bin/bash

# Function to log messages
log() {
    LEVEL=$1
    MESSAGE=$2
    TIMESTAMP=$(date +"%Y-%m-%d %H:%M:%S")
    echo "$TIMESTAMP [$LEVEL] $MESSAGE"
}

# Check if the correct number of arguments is passed
if [ $# -ne 2 ]; then
    log "ERROR" "Usage: patchpilot_updater.sh <old_exe_path> <new_exe_path>"
    exit 1
fi

OLD_PATH=$1
NEW_PATH=$2

# Log the start of the update process
log "INFO" "[*] PatchPilot updater started."
log "INFO" "[*] Waiting 2 seconds for main process to exit..."
sleep 2

# Maximum retries and current retry count
MAX_RETRIES=5
RETRIES=$MAX_RETRIES

# Attempt to replace the old binary with the new one, with retries
while [ $RETRIES -gt 0 ]; do
    if mv "$NEW_PATH" "$OLD_PATH"; then
        log "INFO" "[✔] Successfully replaced binary at '$OLD_PATH'."
        break
    else
        RETRIES=$((RETRIES - 1))
        log "WARN" "[!] Failed to replace binary. Retries left: $RETRIES. Retrying in 1 second..."
        sleep 1
    fi
done

# Check if the replacement was successful
if [ $RETRIES -eq 0 ]; then
    log "ERROR" "[✖] Failed to replace binary '$OLD_PATH' after $MAX_RETRIES attempts. Aborting."
    exit 1
fi

# After the replacement, restart the application
log "INFO" "[*] Attempting to restart application: '$OLD_PATH'"

# Start the application
if "$OLD_PATH" &; then
    log "INFO" "[✔] Update complete. Application restarted successfully."
else
    log "ERROR" "[✖] Failed to restart application '$OLD_PATH'."
    exit 1
fi

# Log the completion of the update process
log "INFO" "[*] PatchPilot update process completed successfully."
