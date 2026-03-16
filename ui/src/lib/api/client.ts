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

// In-memory access token (JWT from the OIDC provider).
// Also persisted to sessionStorage so page reloads don't require re-login.
const SESSION_KEY = "burst_access_token";
let _accessToken: string | null = sessionStorage.getItem(SESSION_KEY);

export function setAccessToken(token: string | null): void {
  _accessToken = token;
  if (token) {
    sessionStorage.setItem(SESSION_KEY, token);
  } else {
    sessionStorage.removeItem(SESSION_KEY);
  }
}

export function getAccessToken(): string | null {
  return _accessToken;
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

  const url = `/api${path}`;
  const response = await fetch(url, { ...options, headers });

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

export async function apiFetchFormData<T>(
  path: string,
  formData: FormData,
): Promise<T> {
  const headers: Record<string, string> = {};
  // Do NOT set Content-Type — let the browser set multipart boundary automatically.

  if (_accessToken) {
    headers["Authorization"] = `Bearer ${_accessToken}`;
  }

  const url = `/api${path}`;
  const response = await fetch(url, {
    method: "POST",
    headers,
    body: formData,
  });

  if (!response.ok) {
    const problem: ProblemDetails = await response.json().catch(() => ({
      type: "urn:burst:error:internal-error",
      title: "Request failed",
      status: response.status,
    }));
    throw new ApiError(problem);
  }

  return response.json();
}
