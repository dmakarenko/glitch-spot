// ABOUTME: Entry for picker overlay windows. Mounts <Picker/> into #root.
import React from "react";
import ReactDOM from "react-dom/client";
import { Picker } from "./Picker";

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <Picker />
  </React.StrictMode>
);
