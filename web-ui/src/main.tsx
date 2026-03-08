import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./styles.css";

// After a deployment, the old JS chunks may be gone. Vite fires this event
// when a dynamic import fails because the chunk hash changed. Reloading
// fetches the new index.html with updated chunk references.
window.addEventListener("vite:preloadError", () => {
  window.location.reload();
});

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
