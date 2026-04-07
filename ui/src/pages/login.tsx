import { useState, type FormEvent } from "react";
import { useNavigate } from "react-router-dom";
import { MessageSquare } from "lucide-react";
import { useAuth } from "../lib/auth/use-auth";
import { ApiError } from "../lib/api/client";
import { getOidcConfig, buildAuthorizationUrl, isLocalLoginEnabled } from "../lib/auth/oidc";
import { Button } from "../components/ui/button";
import { Input } from "../components/ui/input";
import { Spinner } from "../components/ui/spinner";

export function LoginPage() {
  const { login } = useAuth();
  const navigate = useNavigate();
  const oidcConfig = getOidcConfig();
  const showLocalLogin = isLocalLoginEnabled() || !oidcConfig;

  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [isSubmitting, setIsSubmitting] = useState(false);

  async function handleOidcLogin() {
    if (!oidcConfig) return;
    // Generate random state for CSRF protection
    const state = crypto.randomUUID();
    sessionStorage.setItem("oidc_state", state);
    window.location.href = await buildAuthorizationUrl(oidcConfig, state);
  }

  async function handleSubmit(e: FormEvent) {
    e.preventDefault();
    setError("");
    setIsSubmitting(true);

    try {
      await login(username, password);
      navigate("/", { replace: true });
    } catch (err) {
      if (err instanceof ApiError) {
        setError(err.problem.detail ?? err.problem.title);
      } else if (err instanceof Error) {
        setError(err.message);
      } else {
        setError("An unexpected error occurred");
      }
    } finally {
      setIsSubmitting(false);
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-gray-50 px-4 dark:bg-gray-950">
      <div className="w-full max-w-sm space-y-6">
        <div className="text-center">
          <MessageSquare className="mx-auto h-10 w-10 text-indigo-600" />
          <h1 className="mt-3 text-2xl font-bold text-gray-900 dark:text-gray-100">
            Sign in to Burst
          </h1>
        </div>

        {oidcConfig && (
          <div className="space-y-3">
            <Button
              type="button"
              className="w-full"
              onClick={handleOidcLogin}
            >
              <svg
                className="mr-2 h-4 w-4"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <rect x="3" y="11" width="18" height="11" rx="2" ry="2" />
                <path d="M7 11V7a5 5 0 0 1 10 0v4" />
              </svg>
              Sign in with SSO
            </Button>

            {showLocalLogin && (
              <div className="relative">
                <div className="absolute inset-0 flex items-center">
                  <div className="w-full border-t border-gray-200 dark:border-gray-700" />
                </div>
                <div className="relative flex justify-center text-xs">
                  <span className="bg-gray-50 px-2 text-gray-400 dark:bg-gray-950">or</span>
                </div>
              </div>
            )}
          </div>
        )}

        {showLocalLogin && (
          <form onSubmit={handleSubmit} className="space-y-4">
            {error && (
              <div className="rounded-md bg-red-50 p-3 text-sm text-red-700 dark:bg-red-900/30 dark:text-red-400">
                {error}
              </div>
            )}

            <Input
              id="username"
              label="Username"
              type="text"
              autoComplete="username"
              required
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              placeholder="alice"
            />

            <Input
              id="password"
              label="Password"
              type="password"
              autoComplete="current-password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
            />

            <Button
              type="submit"
              className="w-full"
              variant={oidcConfig ? "secondary" : "primary"}
              disabled={isSubmitting}
            >
              {isSubmitting ? <Spinner className="mr-2 h-4 w-4" /> : null}
              {oidcConfig ? "Sign in with credentials" : "Sign in"}
            </Button>
          </form>
        )}
      </div>
    </div>
  );
}
