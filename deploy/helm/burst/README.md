# Burst Helm chart

Burst with its Barbacane gateway, routed by the Gateway API. See the
[Kubernetes guide](../../../docs/guide/kubernetes.md) for what it deploys and
how to install it, and `values.yaml` for every setting.

```bash
helm unittest .                                  # unit tests
helm lint . --strict -f ci/production-values.yaml

# From the repository root, as CI runs them:
yamllint --strict -c deploy/helm/.yamllint.yaml deploy/helm
ct lint --config deploy/helm/ct.yaml             # also requires a version bump
```

Bump `version` in `Chart.yaml` with every change to the chart.
