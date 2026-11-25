# Updating stored source URLs

The seed in `mongo-init/seed_library.js` runs only on an empty MongoDB volume, and it skips the library when that collection already has rows. An existing install keeps the URLs it stored. `backend/scripts/update_source_urls.py` rewrites those URLs in the blocklist library, the `default_config` document, and every account's saved blocklist text. It keeps each line's `|name|category` suffix and leaves comments alone. Dead sources with no replacement are removed. The default is a dry run. Nothing schedules it.

The old-to-new map lives in `backend/app/source_urls.py`. The script reads that module.

Run the commands from the repository root. The Compose service is `zachs-lists`.

```sh
docker compose exec -T zachs-lists env PYTHONPATH=/app/backend \
  python /app/backend/scripts/update_source_urls.py

docker compose exec -T zachs-lists env PYTHONPATH=/app/backend \
  python /app/backend/scripts/update_source_urls.py --apply
```

The dry run prints every change, grouped by collection and account. `--apply` writes the same set. A second dry run prints `Dry run. No changes.`

Rebuild the image first when the script is new to this checkout, so the container has the file:

```sh
docker compose up -d --build zachs-lists
```

Then queue a default build. Sign in as an admin and use Rebuild Default Lists, or call the same route with that session cookie:

```sh
curl -sS -X POST -H "Cookie: session=YOUR_SESSION" \
  http://127.0.0.1:8080/api/admin/rebuild/default
```

With `DEFAULT_SYNC_ENABLED=true`, the next default-list sync replaces `default_config` from the upstream blocklist repository. Library rows and account configs are left as the script wrote them. See [default-list-sync.md](default-list-sync.md).

## Adblock output

Pi-hole v6 can subscribe to the Adblock file (`?format=adblock`). That file keeps `||domain^` rules from adblock-format sources, so subdomain coverage stays in the rule.

Hosts and plain output write the domain from the rule and drop the rule text. A source line `||example.com^` becomes `0.0.0.0 example.com` or `example.com`. Use the Adblock file when the source is in adblock format and the blocker should see the original rule.
