import { useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useAuth } from "../lib/auth/use-auth";
import { getOidcConfig, exchangeCodeForToken } from "../lib/auth/oidc";
import { Spinner } from "../components/ui/spinner";

/**
 * OAuth callback page. Handles the redirect from the OIDC provider,
 * exchanges the authorization code for a token, and redirects to the app.
 */
export function CallbackPage() {
  const navigate = useNavigate();
  const { loginWithToken } = useAuth();
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    async function handleCallback() {
      const config = getOidcConfig();
      if (!config) {
        setError("OIDC is not configured");
        return;
      }

      const params = new URLSearchParams(window.location.search);
      const code = params.get("code");
      const state = params.get("state");
      const savedState = sessionStorage.getItem("oidc_state");

      if (!code) {
        const errorDesc = params.get("error_description") ?? params.get("error") ?? "No authorization code received";
        setError(errorDesc);
        return;
      }

      // Validate state parameter (CSRF protection)
      if (state !== savedState) {
        setError("Invalid state parameter — possible CSRF attack");
        return;
      }
      sessionStorage.removeItem("oidc_state");

      try {
        const token = await exchangeCodeForToken(config, code);
        await loginWithToken(token);
        navigate("/", { replace: true });
      } catch (err) {
        setError(err instanceof Error ? err.message : "Token exchange failed");
      }
    }

    handleCallback();
  }, [navigate, loginWithToken]);

  if (error) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-gray-50 dark:bg-gray-950">
        <div className="max-w-sm text-center">
          <p className="text-lg font-semibold text-red-600">Authentication Failed</p>
          <p className="mt-2 text-sm text-gray-500 dark:text-gray-400">{error}</p>
          <a
            href="/login"
            className="mt-4 inline-block text-sm text-indigo-600 hover:underline"
          >
            Back to login
          </a>
        </div>
      </div>
    );
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-gray-50 dark:bg-gray-950">
      <div className="text-center">
        <Spinner className="mx-auto h-8 w-8 text-indigo-600" />
        <p className="mt-4 text-sm text-gray-500">Completing sign-in...</p>
      </div>
    </div>
  );
}
