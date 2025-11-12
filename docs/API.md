## API Endpoints

### Health Check

```bash
curl http://localhost:8080/health
```

Response:

```json
{
  "status": "healthy",
  "service": "iso-relayer"
}
```

### Get Connection Status

```bash
curl http://localhost:8080/status
```

Response:

```json
{
  "active_connections": 3,
  "connections": [
    {
      "url": "wss://relay.damus.io",
      "status": "Connected"
    }
  ]
}
```

### Get System Metrics (Prometheus format)

```bash
curl http://localhost:8080/metrics
```

### Get Metrics Summary (JSON format)

```bash
curl http://localhost:8080/api/metrics/summary
```

Response:

```json
{
  "events_processed_total": 1000000,
  "duplicates_filtered_total": 250000,
  "events_in_queue": 150,
  "active_connections": 5,
  "memory_usage_mb": 104
}
```

### Get Memory Usage

```bash
curl http://localhost:8080/api/metrics/memory
```

### List All Relays

```bash
curl http://localhost:8080/api/relays
```

### Add Relay

```bash
curl -X POST http://localhost:8080/api/relays/add \
  -H "Content-Type: application/json" \
  -d '{"url": "wss://relay.example.com"}'
```

### Remove Relay

```bash
curl -X DELETE http://localhost:8080/api/relays/remove \
  -H "Content-Type: application/json" \
  -d '{"url": "wss://relay.example.com"}'
```
