#!/bin/bash
set -e

PUID=${PUID:-0}
PGID=${PGID:-0}

# Treat a leading flag as arguments to alchemist
# (supports `docker run <image> --version` etc.).
if [ "${1#-}" != "$1" ]; then
    set -- alchemist "$@"
fi

# Switch to an unprivileged user when either PUID or PGID is set.
# An unset half inherits the other value, so PUID=1000 alone works.
if [ "$PUID" -ne 0 ] || [ "$PGID" -ne 0 ]; then
    if [ "$PUID" -eq 0 ]; then PUID=$PGID; fi
    if [ "$PGID" -eq 0 ]; then PGID=$PUID; fi
    echo "Starting Alchemist with UID: $PUID, GID: $PGID"

    # Take ownership of app data — skip gracefully for read-only mounts
    for dir in /app/config /app/data; do
        if [ -d "$dir" ]; then
            chown -R "$PUID:$PGID" "$dir" 2>/dev/null || \
                echo "Warning: Cannot chown $dir (read-only mount?). Continuing..."
        fi
    done

    # Drop privileges and execute. Numeric uid:gid needs no passwd entry,
    # so arbitrary PUID/PGID values never collide with existing users.
    export HOME=/app
    exec gosu "$PUID:$PGID" "$@"
else
    # Run natively
    exec "$@"
fi
