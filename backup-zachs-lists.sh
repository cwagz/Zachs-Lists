#!/bin/bash
set -euo pipefail

# =============================================================================
# Zach's Lists Backup Script
# Creates a complete backup of .env, ./data, and MongoDB
# =============================================================================

BACKUP_ROOT="./backups"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="${BACKUP_ROOT}/zachs-lists_${TIMESTAMP}"
MONGO_CONTAINER="zachs-lists-db"
DATA_DIR="./data"

echo "=== Zach's Lists Backup ==="
echo "Backup location: ${BACKUP_DIR}"
mkdir -p "${BACKUP_DIR}"

# 1. Backup .env
if [ -f .env ]; then
  echo "→ Backing up .env"
  cp .env "${BACKUP_DIR}/.env"
else
  echo "⚠  No .env found — skipping"
fi

# 2. Backup the data directory (generated lists + cache)
if [ -d "${DATA_DIR}" ]; then
  echo "→ Backing up ./data directory"
  tar -czf "${BACKUP_DIR}/data.tar.gz" -C . data
else
  echo "⚠  No ./data directory found — skipping"
fi

# 3. Backup MongoDB
if docker ps --format '{{.Names}}' | grep -q "^${MONGO_CONTAINER}$"; then
  echo "→ Dumping MongoDB (database: blocklist)..."
  docker exec "${MONGO_CONTAINER}" mongodump --archive --gzip > "${BACKUP_DIR}/mongo.archive.gz"
else
  echo "❌ MongoDB container '${MONGO_CONTAINER}' is not running!"
  echo "   Start it with: docker compose up -d"
  exit 1
fi

# 4. Save docker-compose.yml for reference
cp docker-compose.yml "${BACKUP_DIR}/" 2>/dev/null || true

# Create manifest
cat > "${BACKUP_DIR}/MANIFEST.txt" << EOF
Zach's Lists Backup
Created: $(date)
Hostname: $(hostname)
Mongo container: ${MONGO_CONTAINER}
Database: blocklist
EOF

echo ""
echo "✅ Backup complete!"
echo "   Location: ${BACKUP_DIR}"
echo ""
echo "Contents:"
ls -lh "${BACKUP_DIR}"
echo ""
echo "Tip: To make a single file for easy transfer:"
echo "  tar -czf zachs-lists-backup_${TIMESTAMP}.tar.gz -C ${BACKUP_ROOT} zachs-lists_${TIMESTAMP}"
