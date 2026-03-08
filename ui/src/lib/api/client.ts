export interface ProblemDetails {
  type: string;
  title: string;
  status: number;
  detail?: string;
  errors?: { code: string; message: string; location?: string }[];
}

export class ApiError extends Error {
  public problem: ProblemDetails;
  constructor(problem: ProblemDetails) {
    super(problem.detail ?? problem.title);
    this.problem = problem;
    this.name = "ApiError";
  }
}

// In-memory access token — never written to localStorage.
// Survives navigation but not a full page reload (by design; refresh cookie
// handles re-hydration on startup).
let _accessToken: string | null = null;

export function setAccessToken(token: string | null): void {
  _accessToken = token;
}

export function getAccessToken(): string | null {
  return _accessToken;
}

let _isRefreshing = false;
let _refreshPromise: Promise<boolean> | null = null;

export async function silentRefresh(): Promise<boolean> {
  if (_isRefreshing && _refreshPromise) {
    return _refreshPromise;
  }
  _isRefreshing = true;
  _refreshPromise = (async () => {
    try {
      const response = await fetch("/auth/refresh", {
        method: "POST",
        credentials: "include", // sends the httpOnly refresh_token cookie
      });
      if (!response.ok) {
        _accessToken = null;
        return false;
      }
      const data = await response.json();
      _accessToken = data.accessToken;
      return true;
    } catch {
      _accessToken = null;
      return false;
    } finally {
      _isRefreshing = false;
      _refreshPromise = null;
    }
  })();
  return _refreshPromise;
}

export async function apiFetch<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...((options.headers as Record<string, string>) ?? {}),
  };

  if (_accessToken) {
    headers["Authorization"] = `Bearer ${_accessToken}`;
  }

  let response = await fetch(path, { ...options, headers, credentials: "include" });

  // On 401, attempt a silent refresh and retry once
  if (response.status === 401 && _accessToken) {
    const refreshed = await silentRefresh();
    if (refreshed && _accessToken) {
      headers["Authorization"] = `Bearer ${_accessToken}`;
      response = await fetch(path, { ...options, headers, credentials: "include" });
    }
  }

  if (!response.ok) {
    const problem: ProblemDetails = await response.json().catch(() => ({
      type: "urn:burst:error:internal-error",
      title: "Request failed",
      status: response.status,
    }));
    throw new ApiError(problem);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  return response.json();
}
