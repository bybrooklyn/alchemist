#!/bin/bash
set -e

PUID=${PUID:-0}
PGID=${PGID:-0}

if [ "$PUID" -ne 0 ] && [ "$PGID" -ne 0 ]; then
    echo "Starting Alchemist with UID: $PUID, GID: $PGID"
    
    # Create group and user securely if they don't exist
    if ! getent group alchemist >/dev/null; then
        groupadd -g "$PGID" alchemist
    fi
    if ! getent passwd alchemist >/dev/null; then
        useradd -u "$PUID" -g "$PGID" -s /bin/bash -m -d /app alchemist
    fi
    
    # Take ownership of app data — skip gracefully for read-only mounts
    for dir in /app/config /app/data; do
        if [ -d "$dir" ]; then
            chown -R alchemist:alchemist "$dir" 2>/dev/null || \
                echo "Warning: Cannot chown $dir (read-only mount?). Continuing..."
        fi
    done
    
    # Drop privileges and execute
    exec gosu alchemist "$@"
else
    # Run natively
    exec "$@"
fi
