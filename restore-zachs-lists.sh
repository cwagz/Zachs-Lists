#!/bin/bash
set -euo pipefail

# =============================================================================
# Zach's Lists Restore Script
# Restores .env, ./data, and MongoDB from a backup created by backup-zachs-lists.sh
# =============================================================================

# Usage: ./restore-zachs-lists.sh /path/to/backup/folder
# Example: ./restore-zachs-lists.sh ./backups/zachs-lists_20260820_183000

if [ $# -ne 1 ]; then
  echo "Usage: $0 <path-to-backup-directory>"
  echo "Example: $0 ./backups/zachs-lists_20260820_183000"
  exit 1
fi

BACKUP_DIR="$1"
MONGO_CONTAINER="zachs-lists-db"

if [ ! -d "${BACKUP_DIR}" ]; then
  echo "Error: Backup directory not found: ${BACKUP_DIR}"
  exit 1
fi

echo "=== Zach's Lists Restore ==="
echo "Restoring from: ${BACKUP_DIR}"
echo ""

# Safety check
read -p "This will overwrite MongoDB data and ./data. Continue? (y/N) " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
  echo "Aborted."
  exit 1
fi

# 1. Restore .env
if [ -f "${BACKUP_DIR}/.env" ]; then
  echo "→ Restoring .env"
  cp "${BACKUP_DIR}/.env" .env
else
  echo "⚠  No .env in backup — you will need to create one manually"
fi

# 2. Make sure containers are running
echo "→ Ensuring containers are up..."
docker compose up -d

# Wait for Mongo to be ready
echo "→ Waiting for MongoDB to become ready..."
for i in {1..30}; do
  if docker exec "${MONGO_CONTAINER}" mongosh --quiet --eval 'db.runCommand({ ping: 1 })' &>/dev/null; then
    echo "   MongoDB is ready"
    break
  fi
  if [ $i -eq 30 ]; then
    echo "❌ MongoDB did not become ready in time"
    exit 1
  fi
  sleep 2
done

# 3. Restore MongoDB
if [ -f "${BACKUP_DIR}/mongo.archive.gz" ]; then
  echo "→ Restoring MongoDB (database: blocklist)..."
  docker exec "${MONGO_CONTAINER}" mongosh --quiet --eval 'db.getSiblingDB("blocklist").dropDatabase()' || true
  cat "${BACKUP_DIR}/mongo.archive.gz" | docker exec -i "${MONGO_CONTAINER}" mongorestore --archive --gzip --drop
else
  echo "⚠  No mongo.archive.gz found in backup"
fi

# 4. Restore data directory
if [ -f "${BACKUP_DIR}/data.tar.gz" ]; then
  echo "→ Restoring ./data directory"
  rm -rf ./data
  tar -xzf "${BACKUP_DIR}/data.tar.gz" -C .
else
  echo "⚠  No data.tar.gz found in backup"
fi

# 5. Restart everything so the app picks up the restored data
echo "→ Restarting services..."
docker compose down
docker compose up -d

echo ""
echo "✅ Restore complete!"
echo "Give it 20–30 seconds, then open the web UI."
echo "You should see your previous users, lists, and configurations."
