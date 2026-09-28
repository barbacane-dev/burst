# End-to-end test on kind

Installs the chart on a local [kind](https://kind.sigs.k8s.io/) cluster, with
[Envoy Gateway](https://gateway.envoyproxy.io/) serving the Gateway API, a mock
OIDC provider and, for the storage profile, RustFS. Then runs the k6 smoke
tests through the route.

| File | What it adds |
|------|--------------|
| `kind.yaml` | a one-node cluster |
| `gateway.yaml` | the Gateway the chart's route attaches to |
| `oidc.yaml` | the mock provider, with the users the smoke test signs in as |
| `values.yaml` | the chart with images built from this checkout |
| `s3.yaml`, `values-s3.yaml` | RustFS and its bucket; S3 storage with two server replicas |

## Running it

```bash
# Images: build bin/<arch>/burst for Linux first (see the Dockerfile), then
podman build -t localhost/burst:e2e -f Dockerfile .
podman build -t localhost/burst-gateway:e2e -f docker/Dockerfile.gateway .
podman build -t localhost/burst-nginx:e2e -f docker/Dockerfile.nginx .

export KIND_EXPERIMENTAL_PROVIDER=podman   # omit with Docker
kind create cluster --config deploy/helm/e2e/kind.yaml
for image in burst burst-gateway burst-nginx; do
  podman save "localhost/${image}:e2e" -o "/tmp/${image}.tar"
  kind load image-archive "/tmp/${image}.tar" --name burst
done

helm install eg oci://docker.io/envoyproxy/gateway-helm --version 1.9.1 \
  -n envoy-gateway-system --create-namespace --wait
kubectl create namespace burst
kubectl apply -f deploy/helm/e2e/gateway.yaml -f deploy/helm/e2e/oidc.yaml
helm install chat deploy/helm/burst -n burst -f deploy/helm/e2e/values.yaml --wait
```

kind has no load balancer, so reach the Gateway and the provider through
port-forwards. The provider stamps tokens with the address it is asked on, so
it must be `localhost:<port>`, matching `oidc.issuerOverride`:

```bash
kubectl -n envoy-gateway-system port-forward \
  "$(kubectl -n envoy-gateway-system get svc -l gateway.envoyproxy.io/owning-gateway-name=public -o name)" 18080:80 &
kubectl -n burst port-forward svc/mock-oauth 9099:8080 &
kubectl -n burst port-forward svc/chat-burst-server 13000:3000 &
kubectl -n burst port-forward svc/chat-burst-postgresql 15432:5432 &

cargo run --example seed -- \
  "postgres://burst:$(kubectl -n burst get secret chat-burst -o jsonpath='{.data.postgres-password}' | base64 -d)@127.0.0.1:15432/burst"
GATEWAY_URL=http://127.0.0.1:18080 MOCK_OAUTH_URL=http://localhost:9099 BURST_URL=http://127.0.0.1:13000 \
  k6 run tests/http/smoke.js
```

For the storage profile, `kubectl apply -f deploy/helm/e2e/s3.yaml`, wait for
the `rustfs-bucket` Job, `helm upgrade` with `-f values-s3.yaml` added, and run
`tests/http/smoke-s3.js` as well.

`kind delete cluster --name burst` removes everything.
