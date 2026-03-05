import React, { useState, useEffect, useCallback } from "react";
import { getToken, setToken, verifyToken, login } from "./api";
import { LoginPage } from "./LoginPage";
import { ChatLayout } from "./ChatLayout";

export function App() {
  const [authed, setAuthed] = useState<boolean | null>(null);

  useEffect(() => {
    verifyToken().then((ok) => setAuthed(ok));
  }, []);

  const handleLogin = useCallback(
    async (username: string, password: string) => {
      const token = await login(username, password);
      setToken(token);
      setAuthed(true);
    },
    []
  );

  if (authed === null) {
    return (
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          width: "100%",
          height: "100%",
          color: "var(--color-text-muted)",
        }}
      >
        Loading...
      </div>
    );
  }

  if (!authed) {
    return <LoginPage onLogin={handleLogin} />;
  }

  return <ChatLayout />;
}
