# Product Requirement Document (PRD)
## Project: Glitch Spot (AI-Native Debugging Overlay)
**Version:** 1.0  
**Date:** May 2026  
**Status:** Draft  

---

## 1. Executive Summary & Vision
Glitch Spot is an AI-ready debugging and feedback overlay designed to bridge the gap between human intent, visual context, and automated code generation. Traditional bug reporting tools are built for human-to-human collaboration, forcing engineers to manually parse screenshots, logs, and descriptions. 

Glitch Spot treats the AI coding agent (e.g., Cursor, Claude Code, Windsurf) as the primary consumer. It allows developers, product managers, or QA testers to launch an interactive overlay over a web, desktop, or mobile application, circle elements, edit text live, and instantly expose an AI-optimized, contextual stream. By leveraging the **Model Context Protocol (MCP)** and short-lived passcode links, it turns a visual "vibe check" directly into an automated pull request.

---

## 2. Market Opportunity & Competitive Gap
While robust visual bug tracking systems exist, they possess significant platform gaps and are primarily designed for traditional ticketing workflows (Jira, Linear):

* **Jam.dev:** Dominates browser extensions and has a standalone iOS app, but lacks an embedded native mobile SDK for internal apps and a standalone cross-platform desktop application overlay.
* **Marker.io:** Offers an excellent Web JavaScript SDK widget, but explicitly lacks mobile SDK support or desktop application overlays.
* **Webvizio:** Tailored to live URL proxies and browser extensions, lacking native app deep integrations.

**Glitch Spot's Differentiator:** 1.  True **omnipresence** across Web (SDK), Mobile (iOS/Android native SDKs), and Desktop (Global click-through overlay).
2.  An **AI-first pipeline** where annotations, DOM/View hierarchy data, and developer-defined runtime states are converted directly into LLM prompts via a secure MCP Server architecture.

---

## 3. Target Personas
* **The "Vibe Coding" Developer:** Needs to rapidly prototype or fix bugs by pointing at an interface defect and letting their local IDE write the fix seamlessly.
* **The QA Engineer / Product Manager:** Requires a friction-free method to communicate complex visual glitches alongside background technical payloads without manually building bug reports.

---

## 4. Functional Requirements & Core Epics

### Epic 1: The Interactive Visual Canvas Overlay
The overlay must render seamlessly on top of the host application without interfering with or resetting the underlying application state.

* **FR-1.1: Shape/Area Selector:** Users can click-and-drag to draw bounding boxes (rectangles) or free-form circles around any UI region.
* **FR-1.2: Persistent Annotation Tool:** Users can click anywhere on the canvas or bind text directly to a drawn shape.
* **FR-1.3: Live Modification Engine:** * Markers must feature corner handles allowing users to **resize** and **re-position (drag)** them post-creation.
    * Text annotations must remain fully editable on-canvas until the user finishes the session.
* **FR-1.4: Universal Eraser/Undo:** Single-click deletion of active nodes or multi-step action undo history.

### Epic 2: Advanced Context Harvesting & Metadata Capture
When triggered, the overlay background runner must capture deep diagnostic details silently.

* **FR-2.1: Environmental Telemetry:** Captures OS version, screen resolution, browser/runtime versions, device architecture, network latency, and memory footprints.
* **FR-2.2: Developer Context API (SDK Custom Variables):** Provides a public method allowing engineers to bind custom states to the overlay payload.
    * *Example payload structure:*
        ```json
        {
          "current_view_controller": "ProductDetailsPage",
          "user_authentication_state": "logged_in",
          "active_experiments": ["new_checkout_flow_v2"],
          "application_memory_state": "normal"
        }
        ```
* **FR-2.3: Log Tracing:** Captures the last 100 lines of console logs, network response headers/payloads (excluding encrypted parameters), and native exception stack traces.

### Epic 3: The AI Handshake & Secure Streaming Layer
The data collected cannot simply rest in a static dashboard; it must be optimized for ingest by autonomous AI coding pipelines.

* **FR-3.1: Passcode-Protected Expiration Links:** Generates a unique, secure URL containing the interactive session data. These links are governed by an adjustable expiration TTL (Default: 15 minutes) and secured with an optional 4-digit temporary passcode.
* **FR-3.2: Native Model Context Protocol (MCP) Server:** Glitch Spot runs a lightweight MCP Server. When an IDE agent requests context from a snapshot ID, the server streams clean Markdown paired with structured JSON parameters.
* **FR-3.3: LLM Prompt Synthesis Engine:** Converts noisy human feedback into clear instructions.
    * *System Transformation Example:*
        * *Human Input:* Circles text box and types: "Make this smaller and match the login button color."
        * *Glitch Spot Synthesized Prompt Output:* ```markdown
            ### AI AGENT TASK INSTRUCTION
            - Action: Modify UI element properties.
            - Target Element: Bounding Box [X:140, Y:310, W:280, H:60] on screen 'AuthView'.
            - Desired State: Reduce element dimensions by approximately 15% and synchronize background-color attribute with primary button color hex codes (#3B82F6).
            - Attached Metadata: App utilizes Tailwind CSS and Flexbox rendering.
            ```

---

## 5. Architecture & Form Factors

### 5.1 Web SDK (`@Glitch Spot/web`)
* **Delivery:** Distributed via npm or a lightweight CDN script tag.
* **Mechanics:** Injects a zero-dependency HTML5 `<canvas>` element overlay directly into the DOM tree. Captures visual screenshots using optimized WebGL or MediaDevices screen-capture streams.

### 5.2 Mobile SDK (`Glitch SpotMobile`)
* **Delivery:** Available for iOS (CocoaPods/Swift Package Manager) and Android (Maven/Gradle), along with React Native and Flutter wrappers.
* **Mechanics:** Hooks into the native window UI window layer. Can be configured to trigger via a hardware shake gesture or by listening to the system screenshot button combination.

### 5.3 Desktop Application Overlay (Standalone)
* **Delivery:** Compiled via Tauri or Electron for lightweight Cross-Platform distribution (macOS, Windows, Linux).
* **Mechanics:** Leverages native OS accessibility API permissions to draw a global, transparent click-through window layer over any running target window process on the user's screen.

---

## 6. Non-Functional Requirements (NFRs)

* **NFR-6.1: Security & PII Protection:** The context-harvesting engine must parse text nodes and automatically redact strings matching password fields, credit card regexes, or explicitly tagged secure input views.
* **NFR-6.2: Frame Rate Integrity:** The operational canvas overlay layer must run at or above 60 FPS and must not cause more than a 2% overhead on host application CPU usage during an active session.
* **NFR-6.3: Cold-Start Latency:** The link-generation tool and prompt compilation backend must execute end-to-end within < 800ms of the user hitting "Send to AI".

---

## 7. Future Roadmap Phase Considerations
* **Phase 1.5:** Automated codebase mapping (linking a visual bounding box to the exact file path and line numbers of the component in a linked GitHub repo).
* **Phase 2.0:** Multi-turn live AI debugging, where the coding agent leaves annotation questions *back* on the live overlay for the human user to reply to or clarify in real-time.
