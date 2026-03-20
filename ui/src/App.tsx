import { BrowserRouter, Routes, Route, Navigate } from "react-router-dom";
import { QueryClientProvider } from "@tanstack/react-query";
import { AuthProvider, useAuth } from "./lib/auth/context";
import { ThemeProvider } from "./lib/theme";
import { LoginPage } from "./pages/login";
import { MainLayout, WelcomeView } from "./components/layout/main-layout";
import { ChannelPage } from "./pages/channel";
import { SettingsPage } from "./pages/settings";
import { AdminPage } from "./pages/admin";
import { CallbackPage } from "./pages/callback";
import { Spinner } from "./components/ui/spinner";
import { queryClient } from "./lib/query-client";
import type { ReactNode } from "react";

function RequireAuth({ children }: { children: ReactNode }) {
  const { user, isLoading } = useAuth();

  if (isLoading) {
    return (
      <div className="flex h-screen items-center justify-center">
        <Spinner className="h-8 w-8 text-indigo-600" />
      </div>
    );
  }

  if (!user) {
    return <Navigate to="/login" replace />;
  }

  return children;
}

function AppRoutes() {
  const { user, isLoading } = useAuth();

  if (isLoading) {
    return (
      <div className="flex h-screen items-center justify-center">
        <Spinner className="h-8 w-8 text-indigo-600" />
      </div>
    );
  }

  return (
    <Routes>
      <Route
        path="/login"
        element={user ? <Navigate to="/" replace /> : <LoginPage />}
      />
      <Route path="/callback" element={<CallbackPage />} />
      <Route
        path="/"
        element={
          <RequireAuth>
            <MainLayout />
          </RequireAuth>
        }
      >
        <Route index element={<WelcomeView />} />
        <Route path="channels/:channelId" element={<ChannelPage />} />
        <Route path="settings" element={<SettingsPage />} />
        <Route path="admin" element={<AdminPage />} />
      </Route>
    </Routes>
  );
}

export default function App() {
  return (
    <ThemeProvider>
      <QueryClientProvider client={queryClient}>
        <BrowserRouter>
          <AuthProvider>
            <AppRoutes />
          </AuthProvider>
        </BrowserRouter>
      </QueryClientProvider>
    </ThemeProvider>
  );
}
