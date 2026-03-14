import React, { useState, FormEvent, useEffect } from "react";
import { Lock, User, Terminal, ArrowRight, Loader2 } from "lucide-react";

interface Props {
  onLogin: (username: string, password: string) => Promise<void>;
  appName?: string;
}

export function LoginPage({ onLogin, appName = "opman" }: Props) {
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);
  const [focused, setFocused] = useState<"user" | "pass" | null>(null);

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault();
    setError("");
    setLoading(true);
    try {
      await onLogin(username, password);
    } catch {
      setError("Invalid username or password");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="login-container">
      {/* Animated background grid */}
      <div className="login-bg-grid" />
      <div className="login-bg-glow" />

      <form className="login-box" onSubmit={handleSubmit}>
        {/* Logo / branding */}
        <div className="login-brand">
          <div className="login-logo">
            <Terminal size={24} strokeWidth={2.5} />
          </div>
          <h1>{appName}</h1>
          <div className="subtitle">AI-Powered Development Environment</div>
        </div>

        {/* Username field */}
        <div className={`login-field ${focused === "user" ? "focused" : ""}`}>
          <label>
            <User size={12} />
            Username
          </label>
          <input
            type="text"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            onFocus={() => setFocused("user")}
            onBlur={() => setFocused(null)}
            autoFocus
            autoComplete="username"
            placeholder="Enter username"
          />
        </div>

        {/* Password field */}
        <div className={`login-field ${focused === "pass" ? "focused" : ""}`}>
          <label>
            <Lock size={12} />
            Password
          </label>
          <input
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            onFocus={() => setFocused("pass")}
            onBlur={() => setFocused(null)}
            autoComplete="current-password"
            placeholder="Enter password"
          />
        </div>

        <button className="login-btn" type="submit" disabled={loading || !username || !password}>
          {loading ? (
            <>
              <Loader2 size={14} className="spinning" />
              Authenticating...
            </>
          ) : (
            <>
              Sign In
              <ArrowRight size={14} />
            </>
          )}
        </button>

        {error && <div className="login-error">{error}</div>}

        <div className="login-footer">
          <kbd>Enter</kbd> to sign in
        </div>
      </form>
    </div>
  );
}
