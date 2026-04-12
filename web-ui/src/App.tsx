import React, { useState, useEffect, useCallback } from "react";
import { verifyToken, login, fetchBootstrap } from "./api";
import { LoginPage } from "./LoginPage";
import { ChatLayout } from "./ChatLayout";
import { ErrorBoundary } from "./ErrorBoundary";
import { getPersistedThemeMode, applyThemeMode } from "./ThemeSelectorModal";
import { applyThemeToCss } from "./utils/theme";
import { initAppearance, resolveThemeColors, storeThemePair } from "./utils/appearance";

export function App() {
  const [authed, setAuthed] = useState<boolean | null>(null);
  const [bootstrapReady, setBootstrapReady] = useState(false);
  // Use document.title (server-patched from tunnel hostname) as the
  // synchronous default so the login page shows the correct name on
  // first paint, before the async bootstrap fetch resolves.
  const [appName, setAppName] = useState<string>(
    () => document.title || "opman"
  );

  // Apply persisted theme mode (glassy/flat) + appearance (system/light/dark)
  // immediately on mount, before auth check completes.
  useEffect(() => {
    applyThemeMode(getPersistedThemeMode());
    initAppearance();
  }, []);

  // Fetch bootstrap + verify token in parallel.  Both must finish
  // before we render anything beyond the loader so the login page
  // never flashes default colours before switching to the real theme.
  useEffect(() => {
    const appearance = initAppearance();
    const bootstrap = fetchBootstrap().then((data) => {
      if (data.theme) {
        storeThemePair(data.theme, appearance);
        applyThemeToCss(resolveThemeColors(data.theme, appearance));
      }
      if (data.instance_name) setAppName(data.instance_name);
    });

    const auth = verifyToken().then((ok) => setAuthed(ok));

    Promise.allSettled([bootstrap, auth]).then(() => {
      setBootstrapReady(true);
    });
  }, []);

  const handleLogin = useCallback(
    async (username: string, password: string) => {
      await login(username, password);
      // The server sets an HttpOnly cookie — no client-side token storage needed.
      setAuthed(true);
    },
    []
  );

  // Block rendering until both bootstrap (theme) and auth check resolve.
  if (!bootstrapReady || authed === null) {
    return <AppLoader />;
  }

  if (!authed) {
    return <LoginPage onLogin={handleLogin} appName={appName} />;
  }

  return (
    <ErrorBoundary>
      <ChatLayout />
    </ErrorBoundary>
  );
}

/** Minimal full-screen loader shown while bootstrap + auth resolve. */
function AppLoader() {
  return (
    <div className="app-loader">
      <div className="app-loader-spinner" />
    </div>
  );
}
