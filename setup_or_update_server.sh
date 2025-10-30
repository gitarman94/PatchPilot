# Download latest release from GitHub (no token required for public repo)
TMPDIR=$(mktemp -d)
trap 'rm -rf "$TMPDIR"' EXIT
cd "$TMPDIR"

curl -L "$ZIP_URL" -o latest.zip

# Check if the ZIP file was downloaded successfully
if [[ ! -f latest.zip ]]; then
    echo "❌ Download failed! Please check the URL."
    exit 1
fi

unzip -o latest.zip

EXTRACTED_DIR=$(find . -maxdepth 1 -type d -name "${GITHUB_REPO}-*")
cp -r "${EXTRACTED_DIR}/"* "${APP_DIR}/"

# Move files from /opt/patchpilot_server/patchpilot_server/ to /opt/patchpilot_server/
mv /opt/patchpilot_server/patchpilot_server/* /opt/patchpilot_server/

# Remove the empty directory
rm -rf /opt/patchpilot_server/patchpilot_server/

# Navigate to the correct directory where Cargo.toml is located
cd "${APP_DIR}/patchpilot_server"

# Set up SQLite database
SQLITE_DB="${APP_DIR}/patchpilot.db"
touch "$SQLITE_DB"
chown patchpilot:patchpilot "$SQLITE_DB"
chmod 600 "$SQLITE_DB"

# Set up log file and permissions
touch /opt/patchpilot_server/server.log
chown patchpilot:patchpilot /opt/patchpilot_server/server.log
chmod 644 /opt/patchpilot_server/server.log

# Setup admin token
TOKEN_FILE="${APP_DIR}/admin_token.txt"
ENV_FILE="${APP_DIR}/admin_token.env"
if [[ ! -f "$TOKEN_FILE" ]]; then
    ADMIN_TOKEN=$(openssl rand -base64 32 | tr -d '=+/')
    echo "$ADMIN_TOKEN" > "$TOKEN_FILE"
    chmod 600 "$TOKEN_FILE"
else
    ADMIN_TOKEN=$(cat "$TOKEN_FILE")
fi
printf "ADMIN_TOKEN=%s\n" "$ADMIN_TOKEN" > "$ENV_FILE"
chmod 600 "$ENV_FILE"

# Set ownership of the entire directory to patchpilot
chown -R patchpilot:patchpilot "${APP_DIR}"

# Build the Rust application
echo "🔨 Building the Rust application..."
cargo build --release
