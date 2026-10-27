// ABOUTME: Entry point for the overlay window's React app.
// ABOUTME: Mounts the Overlay component into the #root element.
import React from "react";
import ReactDOM from "react-dom/client";
import { Overlay } from "./Overlay";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Overlay />
  </React.StrictMode>
);
