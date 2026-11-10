#!/usr/bin/env sh
# install_linux.sh - install the cannon server as a systemd service and the
# cannon client for the user that ran this script with sudo or doas.
#
# Change the defaults below if you need to, then run one of:
#   sudo ./scripts/install_linux.sh
#   doas ./scripts/install_linux.sh

set -eu

# --- configuration --------------------------------------------------------

APP="cannon"
SERVER_BIN="cannon-server"       # installed server binary name
CLIENT_BIN="cannon"              # installed client binary name

SERVICE_USER="cannon"
SERVICE_GROUP="cannon"

BIN_DIR="target/release"         # where cargo puts the built binaries
SERVER_DIR="/usr/local/bin"      # server install directory
CLIENT_USER="${SUDO_USER:-${DOAS_USER:-}}"  # user that gets the client
CLIENT_DIR=""                    # empty: ~USER/.local/bin

DATA_DIR="/var/lib/$APP"
CONFIG_DIR="/etc/$APP"
ENV_FILE="$CONFIG_DIR/$APP.env"
UNIT_FILE="/etc/systemd/system/$APP.service"
DESCRIPTION="Fire off your files with this file cannon"

BIND_ADDRESS="0.0.0.0"
PORT="3000"
API_KEY=""                       # empty: generate one if ENV_FILE is missing

# --------------------------------------------------------------------------

die() {
    printf '%s: %s\n' "${0##*/}" "$*" >&2
    exit 1
}

[ $# -eq 0 ] || die "this script takes no arguments; edit the variables at the top instead"
[ "$(id -u)" -eq 0 ] || die "run with sudo, doas, or as root"
command -v systemctl >/dev/null 2>&1 ||
    die "systemctl not found; this installer supports systemd systems only"

SERVER_SRC="$BIN_DIR/cannon_server"
CLIENT_SRC="$BIN_DIR/cannon"

[ -f "$SERVER_SRC" ] ||
    die "server binary not found: $SERVER_SRC (run 'cargo build --release --bins' first)"
[ -f "$CLIENT_SRC" ] ||
    die "client binary not found: $CLIENT_SRC (run 'cargo build --release --bins' first)"

# The client belongs to the human who invoked sudo/doas, not to root.
[ -n "$CLIENT_USER" ] ||
    die "cannot determine the client user; run with sudo/doas or set CLIENT_USER at the top"
getent passwd "$CLIENT_USER" >/dev/null 2>&1 || die "unknown user: $CLIENT_USER"
CLIENT_HOME=$(getent passwd "$CLIENT_USER" | cut -d: -f6)
[ -n "$CLIENT_HOME" ] || die "user $CLIENT_USER has no home directory"
CLIENT_GROUP=$(getent group "$(id -g "$CLIENT_USER")" | cut -d: -f1)
[ -n "$CLIENT_GROUP" ] || die "cannot determine the primary group of $CLIENT_USER"
CLIENT_DIR=${CLIENT_DIR:-$CLIENT_HOME/.local/bin}

# System account for the service: no home directory, no interactive login.
NOLOGIN="/usr/sbin/nologin"
[ -x "$NOLOGIN" ] || NOLOGIN="/sbin/nologin"
[ -x "$NOLOGIN" ] || NOLOGIN="/bin/false"

if ! getent group "$SERVICE_GROUP" >/dev/null 2>&1; then
    groupadd --system "$SERVICE_GROUP"
fi
if ! getent passwd "$SERVICE_USER" >/dev/null 2>&1; then
    useradd --system --gid "$SERVICE_GROUP" --no-create-home \
        --home-dir "$DATA_DIR" --shell "$NOLOGIN" "$SERVICE_USER"
fi

[ -d "$SERVER_DIR" ] || install -d -o root -g root -m 0755 "$SERVER_DIR"
install -d -o "$SERVICE_USER" -g "$SERVICE_GROUP" -m 0750 "$DATA_DIR"
install -d -o "$SERVICE_USER" -g "$SERVICE_GROUP" -m 0750 "$DATA_DIR/data"
install -d -o root -g root -m 0755 "$CONFIG_DIR"
install -d -o root -g root -m 0755 "$(dirname "$UNIT_FILE")"
install -d -o "$CLIENT_USER" -g "$CLIENT_GROUP" -m 0755 \
    "$(dirname "$CLIENT_DIR")" "$CLIENT_DIR"

# Keep an existing key; otherwise use API_KEY or generate a random one.
if [ -f "$ENV_FILE" ]; then
    printf 'Keeping existing %s\n' "$ENV_FILE"
else
    if [ -z "$API_KEY" ]; then
        if command -v openssl >/dev/null 2>&1; then
            API_KEY=$(openssl rand -hex 32)
        else
            API_KEY=$(od -An -N32 -tx1 /dev/urandom | tr -d ' \n')
        fi
    fi
    (umask 077; printf 'CANNON_API_KEY=%s\n' "$API_KEY" > "$ENV_FILE")
fi
chown root:root "$ENV_FILE"
chmod 0600 "$ENV_FILE"

# Replace binaries atomically so a running service keeps its old inode.
install_atomic() { # owner group src dest
    dir=$(dirname "$4")
    tmp="$dir/.${4##*/}.new"
    install -o "$1" -g "$2" -m 0755 "$3" "$tmp"
    mv -f "$tmp" "$4"
}

install_atomic root root "$SERVER_SRC" "$SERVER_DIR/$SERVER_BIN"
install_atomic "$CLIENT_USER" "$CLIENT_GROUP" "$CLIENT_SRC" "$CLIENT_DIR/$CLIENT_BIN"

# Remove a client left behind by an older, system-wide install.
if [ "$CLIENT_DIR" != "$SERVER_DIR" ] && [ -e "$SERVER_DIR/$CLIENT_BIN" ]; then
    rm -f "$SERVER_DIR/$CLIENT_BIN"
    printf 'Removed old system-wide client %s\n' "$SERVER_DIR/$CLIENT_BIN"
fi

# A cargo-installed copy in ~/.cargo/bin usually comes first in PATH.
if [ -x "$CLIENT_HOME/.cargo/bin/$CLIENT_BIN" ] && [ "$CLIENT_HOME/.cargo/bin" != "$CLIENT_DIR" ]; then
    printf 'warning: %s also contains %s and may shadow %s\n' \
        "$CLIENT_HOME/.cargo/bin" "$CLIENT_BIN" "$CLIENT_DIR/$CLIENT_BIN"
    printf '         remove it with: cargo uninstall %s\n' "$APP"
fi

cat > "$UNIT_FILE" <<EOF
[Unit]
Description=$DESCRIPTION
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$SERVICE_USER
Group=$SERVICE_GROUP
StateDirectory=$APP
StateDirectoryMode=0750
EnvironmentFile=$ENV_FILE
Environment=CANNON_DATA_DIR=$DATA_DIR
Environment=CANNON_BIND=$BIND_ADDRESS:$PORT
ExecStart=$SERVER_DIR/$SERVER_BIN
Restart=on-failure
RestartSec=5
UMask=0027
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes

[Install]
WantedBy=multi-user.target
EOF

chmod 0644 "$UNIT_FILE"
systemctl daemon-reload
systemctl reset-failed "$APP.service" 2>/dev/null || true
systemctl enable "$APP.service"
systemctl restart "$APP.service"

printf '\nInstalled %s\n' "$APP"
printf '  server binary: %s\n' "$SERVER_DIR/$SERVER_BIN"
printf '  client binary: %s (for %s)\n' "$CLIENT_DIR/$CLIENT_BIN" "$CLIENT_USER"
printf '  data dir:      %s\n' "$DATA_DIR"
printf '  config:        %s\n' "$ENV_FILE"
printf '  unit:          %s\n' "$UNIT_FILE"

printf '\nNext steps:\n'
printf '  systemctl status %s.service\n' "$APP"
printf '  # as %s:\n' "$CLIENT_USER"
printf '  export CANNON_SERVER=http://<host>:%s\n' "$PORT"
printf '  export CANNON_API_KEY=<key from %s>\n' "$ENV_FILE"
printf '  %s <file>\n' "$CLIENT_BIN"
