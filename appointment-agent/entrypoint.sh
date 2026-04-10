#!/bin/sh
# Version: 0.1.1
# Description: Docker entrypoint to ensure config existence and start the agent.

set -e

CONFIG_DIR="/root/configs"
CONFIG_FILE="$CONFIG_DIR/config.yaml"
DEFAULT_FILE="$CONFIG_DIR/default_config.yaml"

# Ensure the config directory exists
mkdir -p "$CONFIG_DIR"

# Initialize config.yaml from default if it doesn't exist or is empty
if [ ! -s "$CONFIG_FILE" ]; then
    echo "Initializing config.yaml from default..."
    if [ -f "$DEFAULT_FILE" ]; then
        cp "$DEFAULT_FILE" "$CONFIG_FILE"
    else
        echo "Error: Default config not found at $DEFAULT_FILE"
        exit 1
    fi
fi

echo "Starting Appointment Agent..."
# Exec into the agent to ensure signals (SIGTERM) are handled by the process
exec /usr/local/bin/agent --config "$CONFIG_FILE" --web
