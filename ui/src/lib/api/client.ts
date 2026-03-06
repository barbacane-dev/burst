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

export async function apiFetch<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const token = localStorage.getItem("access_token");

  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...((options.headers as Record<string, string>) ?? {}),
  };

  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  const response = await fetch(path, { ...options, headers });

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
