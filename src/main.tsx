// ABOUTME: Entry point for the main Glitch Spot window.
// ABOUTME: Mounts the React app into the #root DOM element.
import React from "react";
import ReactDOM from "react-dom/client";
import { App } from "./App";
import "./styles/globals.css";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
