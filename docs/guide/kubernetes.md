# Kubernetes

The Helm chart in `deploy/helm/burst` runs Burst with its Barbacane gateway, routed by the Gateway API.

## What It Deploys

```text
Gateway API route ─┬─ /api, /ws → gateway (Barbacane: OIDC, ACL, rate limits) → server (Burst) → PostgreSQL
                   └─ /         → web (nginx, the SPA)
                                   server → storage gateway (Barbacane, holds the S3 credentials) → S3
```

| Component | Kind | Notes |
|-----------|------|-------|
| server | Deployment | Burst. Runs migrations at startup; replicas coordinate through a PostgreSQL lock. |
| gateway | Deployment | Barbacane with the compiled API artifact. Authenticates every `/api` and `/ws` request. |
| web | Deployment | nginx serving the SPA and its runtime `env.js`. |
| storage | Deployment | Barbacane with the storage artifact, only with `storage.backend: s3`. |
| postgresql | StatefulSet | Only with `postgresql.enabled: true`, for trying Burst out. |

The server believes the identity headers the gateway sets, so a NetworkPolicy lets only the gateway reach its API port. Keep `networkPolicy.enabled: true` on a cluster whose network plugin enforces policies.

## Requirements

- Kubernetes 1.30 or later, with a [Gateway API](https://gateway-api.sigs.k8s.io/) implementation and a Gateway to attach the route to.
- An OIDC provider, with a client for the SPA whose redirect URI is `https://<your hostname>/callback`.
- PostgreSQL, unless you use the bundled one to try Burst out.
- For more than one server replica: an S3-compatible bucket.

The images are on `ghcr.io/barbacane-dev`. While they are private, create a pull secret and list it in `imagePullSecrets`.

## Trying It Out

A single server with the bundled PostgreSQL and files on a volume, reached with a port-forward:

```bash
helm install chat deploy/helm/burst \
  --set oidc.issuerUrl=https://id.example.com/realms/burst \
  --set postgresql.enabled=true \
  --set httpRoute.enabled=false

kubectl port-forward svc/chat-burst-web 8080:8080
```

The bundled PostgreSQL has no backups or failover. Its password is generated on install and kept across upgrades.

## Production

```yaml
oidc:
  issuerUrl: https://id.example.com/realms/burst
database:
  existingSecret: burst-database    # key "url": postgres://user:password@host:5432/burst
storage:
  backend: s3
  s3:
    endpoint: https://s3.eu-west-1.amazonaws.com
    region: eu-west-1
    bucket: burst-files
    existingSecret: burst-bucket    # keys "accessKeyId" and "secretAccessKey"
server:
  replicas: 3
httpRoute:
  parentRefs:
    - name: public
      namespace: gateway-system
  hostnames:
    - chat.example.com
serviceMonitor:
  enabled: true
```

With more than one server replica the chart switches the event broker to PostgreSQL `LISTEN/NOTIFY`, so every replica sees every event. Local storage refuses more than one replica, since each pod would keep its own files.

## Values

The chart refuses combinations that would install but not work: no OIDC issuer, no database, local storage with several replicas, S3 without an endpoint or credentials, a route with no Gateway to attach to. See `deploy/helm/burst/values.yaml` for every value, each with its default and what it does.

## Rolling updates

A stopping pod keeps serving for `drainSeconds` (5 by default) while its address leaves the Service's endpoints, so a rolling update sends no connection to a pod that has already exited. It uses the `preStop` sleep action, which is why the chart needs Kubernetes 1.30 or later.

## Monitoring

The server and the gateway serve Prometheus metrics on their admin ports (3001 and 8090). Set `serviceMonitor.enabled: true` with the Prometheus Operator, and `networkPolicy.metricsNamespaceSelector` to let the monitoring namespace reach those ports.
