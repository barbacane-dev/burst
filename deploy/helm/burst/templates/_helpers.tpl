{{- define "burst.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "burst.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/* The name of one component's resources: <fullname>-<component>. */}}
{{- define "burst.componentName" -}}
{{- printf "%s-%s" (include "burst.fullname" .root) .component | trunc 63 | trimSuffix "-" }}
{{- end }}

{{- define "burst.labels" -}}
helm.sh/chart: {{ printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" }}
app.kubernetes.io/part-of: {{ include "burst.name" . }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{- define "burst.selectorLabels" -}}
app.kubernetes.io/name: {{ include "burst.name" .root }}
app.kubernetes.io/instance: {{ .root.Release.Name }}
app.kubernetes.io/component: {{ .component }}
{{- end }}

{{- define "burst.componentLabels" -}}
{{ include "burst.labels" .root }}
{{ include "burst.selectorLabels" . }}
{{- end }}

{{- define "burst.image" -}}
{{- printf "%s:%s" .image.repository (default .root.Chart.AppVersion .image.tag) }}
{{- end }}

{{- define "burst.secretName" -}}
{{- include "burst.fullname" . }}
{{- end }}

{{/*
A value generated once and kept: the one already in the chart's Secret when
there is one, a new random one otherwise.
*/}}
{{- define "burst.keptSecret" -}}
{{- $existing := lookup "v1" "Secret" .root.Release.Namespace (include "burst.secretName" .root) }}
{{- if and $existing (hasKey $existing.data .key) }}
{{- index $existing.data .key | b64dec }}
{{- else }}
{{- randAlphaNum 40 }}
{{- end }}
{{- end }}

{{- define "burst.postgresPassword" -}}
{{- default (include "burst.keptSecret" (dict "root" . "key" "postgres-password")) .Values.postgresql.password }}
{{- end }}

{{/* The Secret and key holding the storage gateway's API key. */}}
{{- define "burst.storageApiKeyRef" -}}
{{- if .Values.storageApiKey.existingSecret }}
name: {{ .Values.storageApiKey.existingSecret }}
key: apiKey
{{- else }}
name: {{ include "burst.secretName" . }}
key: storage-api-key
{{- end }}
{{- end }}

{{- define "burst.redirectUri" -}}
{{- if .Values.oidc.redirectUri }}
{{- .Values.oidc.redirectUri }}
{{- else if .Values.httpRoute.hostnames }}
{{- printf "https://%s/callback" (first .Values.httpRoute.hostnames) }}
{{- end }}
{{- end }}

{{/* Refuses a combination that would install but not work. */}}
{{- define "burst.validate" -}}
{{- if not .Values.oidc.issuerUrl }}
{{- fail "oidc.issuerUrl is required: the gateway validates sign-in tokens against it" }}
{{- end }}
{{- if and (not .Values.database.existingSecret) (not .Values.postgresql.enabled) }}
{{- fail "set database.existingSecret to a Secret holding the PostgreSQL URL, or postgresql.enabled=true to try Burst out" }}
{{- end }}
{{- if not (has .Values.storage.backend (list "local" "s3")) }}
{{- fail (printf "storage.backend must be local or s3, not %q" .Values.storage.backend) }}
{{- end }}
{{- if and (eq .Values.storage.backend "local") (gt (int .Values.server.replicas) 1) }}
{{- fail "storage.backend=local keeps files on one pod's volume; use storage.backend=s3 for more than one server replica" }}
{{- end }}
{{- if eq .Values.storage.backend "s3" }}
{{- if not .Values.storage.s3.endpoint }}
{{- fail "storage.s3.endpoint is required with storage.backend=s3" }}
{{- end }}
{{- if not .Values.storage.s3.existingSecret }}
{{- fail "storage.s3.existingSecret is required with storage.backend=s3: a Secret holding the bucket's access key" }}
{{- end }}
{{- end }}
{{- if and .Values.httpRoute.enabled (not .Values.httpRoute.parentRefs) }}
{{- fail "httpRoute.parentRefs must name the Gateway the route attaches to, or set httpRoute.enabled=false" }}
{{- end }}
{{- end }}

{{/*
A pod behind a Service keeps serving for drainSeconds after it is told to stop,
while its address leaves the Service's endpoints; a connection sent to it after
it exited would hang until the caller's timeout.
*/}}
{{- define "burst.lifecycle" -}}
{{- if gt (int .Values.drainSeconds) 0 }}
lifecycle:
  preStop:
    sleep:
      seconds: {{ int .Values.drainSeconds }}
{{- end }}
{{- end }}
