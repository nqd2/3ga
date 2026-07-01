# Augmented Gaussian GUI Design System & Redesign Specification (Viettel Telecom)

## Context and Goals
This document defines the implementation-ready design system and visual guidelines for the **Augmented Gaussian Calibration Tool GUI** under the visual brand framework of **Viettel Telecom**. 

The Augmented Gaussian application is a 3D Gaussian Splatting (3DGS) calibration utility that processes, filters, and reconstructs 3DGS point clouds into structured, collision-aware geometry (meshes and navmeshes) for Augmented Reality (AR) applications.

The GUI implementation is built using the **Astryx Component Library (`@astryxdesign/core`)**. All interactive widgets, layouts, and forms are derived from Astryx components, with custom branding applied through theme values and semantic token definitions.

**Design Intent Statement:**
This design system maps Viettel Telecom’s brand foundations onto the Astryx Component Library to create a professional, accessible, and high-density 3D calibration GUI.

---

## Design Tokens and Foundations

### Typography
The interface uses a clean, implementation-oriented sans-serif scale. All textual elements must map to these tokens exactly:
- **Primary Font Family:** `font.family.primary = Roboto`
- **Fallback Font Stack:** `font.family.stack = Roboto, sans-serif`
- **Base Font Size:** `font.size.base = 14px`
- **Base Font Weight:** `font.weight.base = 400`
- **Base Line Height:** `font.lineHeight.base = 22.001px`
- **Typography Scale:**
  - `font.size.xs = 0px` (Strictly reserved for screen-reader-only `sr-only` visually hidden tags)
  - `font.size.sm = 14px` (Secondary text, field helper texts, button labels)
  - `font.size.md = 15px` (Primary labels, input text values)
  - `font.size.lg = 16px` (Card subheadings, secondary action groups)
  - `font.size.xl = 17px` (Section titles, panel headers)
  - `font.size.2xl = 18px` (Workspace headers)
  - `font.size.3xl = 19px` (Main panel status indicators)
  - `font.size.4xl = 20px` (Main application title)

### Color Palette (Semantic Tokens)
All components must use these semantic references rather than raw color hexes:
- **Primary Text:** `color.text.primary = #2c2f31`
- **Secondary Text:** `color.text.secondary = #333333`
- **Base Canvas/Body Background:** `color.surface.base = #000000` (Provides a deep environment backdrop)
- **Muted Surface / Panels:** `color.surface.muted = #ffffff` (Provides contrast for layout card modules)
- **Accent Surface (Viettel Red):** `color.surface.raised = #d11313` (Main accent, primary buttons, and highlight states)
- **Strong Border:** `color.border.strong = #eaeaea`
- **Interactive Focus Outline:** `color.border.focus = #d11313`
- **Non-Text WCAG High Contrast (Viewport overlays):**
  - Floor plane calibration normal: `#d11313` (Accent red)
  - Floor calibration points: `#ffd159` (Yellow, meets 3:1 contrast against dark viewport)
  - Scale calibration points: `#73b3ff` (Blue, meets 3:1 contrast against dark viewport)

### Spacing Scale
Layout spacing must exclusively use the following spacing tokens:
- `space.1 = 2px`
- `space.2 = 4px`
- `space.3 = 5px`
- `space.4 = 8px`
- `space.5 = 9px`
- `space.6 = 10px`
- `space.7 = 11px`
- `space.8 = 12px`

### Radii, Shadow, and Motion Tokens
- **Border Radii:**
  - `radius.xs = 8px` (Inputs, text fields, minor controls)
  - `radius.sm = 10px` (Buttons, small cards)
  - `radius.md = 12px` (Primary configuration cards)
  - `radius.lg = 16px` (Panels and modal dialogs)
  - `radius.xl = 22px` (Large banner segments)
  - `radius.2xl = 50px` (Circular pill buttons, checkmarks)
- **Motion Durations:**
  - `motion.duration.instant = 200ms` (Hover transitions, active button down states)
  - `motion.duration.fast = 300ms` (Dropdown animations, state fades)
  - `motion.duration.normal = 500ms` (Loading skeleton fades, progress transitions)

---

## Component-Level Rules & Astryx Mapping

The application layout is structured using the following Astryx core components:

### 1. Workspace Layout (Astryx AppShell & Panels)
- **Astryx Compositions:** `XDSAppShell` / `XDSLayout` structure with custom styling.
- **Rules:**
  - Control panels and sidebars must use a `1px` border of `color.border.strong`.
  - Spacing between panels must be defined by `space.8`.
  - On mobile/tablet screens ($\le 1100px$), panels must flow vertically.

### 2. Sidebar Config Modules (Astryx Card, TextInput, NumberInput, Selector)
- **Astryx Compositions:** Configuration parameters are grouped using Astryx `Card` containing form widgets.
- **Rules:**
  - **Cards:** Must render with border `1px solid color.border.strong` and background `color.surface.muted`. Card padding must use `space.6` spacing.
  - **Inputs (TextInput / NumberInput):**
    - Input values must display in `font.size.md`.
    - Active borders must transition using `motion.duration.instant` and outline focus using `color.border.focus`.
    - Custom validation error messages must use a helper text color matching `color.surface.raised`.
  - **Dropdown Selection (Selector):**
    - Options dropdown menu must support full keyboard navigation (arrows and `Enter` selection).
    - Custom labels of the `Selector` component must be managed with `isLabelHidden` when wrapper headers are present.

### 3. Action Buttons (Astryx Button)
- **Astryx Compositions:** `Button` components for background job triggers.
- **Rules:**
  - **Primary Action (Bake Geometry):** Must use variant `primary` mapping background to `color.surface.raised` and text to `color.surface.muted`. Border radius must match `radius.2xl`.
  - **Secondary Actions (Load Source, Cancel, Browse):** Must use variant `secondary` or icon-only buttons mapping to neutral background structures.
  - **States:**
    - *Default:* Base styling.
    - *Hover:* Background color shifts dynamically over `motion.duration.instant`.
    - *Focus-visible:* Keyboard focus must show a prominent outer outline of `color.border.focus`.
    - *Disabled:* Buttons must have `opacity: 0.5`, background `#eaeaea`, and pointer-events `none`.
    - *Loading:* Shows the loading spin state within the `Button` boundaries.

### 4. 3D Viewport Canvas & Legend Overlays
- **Astryx Compositions:** Utilizes custom canvas overlay elements styled with Astryx typography tokens.
- **Rules:**
  - Viewport background must be `color.surface.base`.
  - Calibration points (floor and scale endpoints) must utilize high-contrast indicators that are visible against WebGL rendering.
  - Interactive axes must highlight the active coordinate axis in `color.surface.raised` (#d11313).

### 5. Log Console (Astryx Text)
- **Astryx Compositions:** Standardized box rendering monospaced status logs.
- **Rules:**
  - Text lines must use `font.family.stack = ui-monospace, SFMono-Regular, monospace` and sizing `font.size.sm`.
  - Color lines must utilize high-contrast white/light-grey text (`#eaeaea`) with `color.surface.raised` red highlighting for errors.

---

## Accessibility Requirements (WCAG 2.2 AA)

To satisfy the WCAG 2.2 AA target, all Astryx configurations must pass these testable criteria:

### Contrast Constraints
- **Normal Text (under 18pt):** Text of `color.text.primary` (#2c2f31) or `color.text.secondary` (#333333) on `color.surface.muted` (#ffffff) must maintain a contrast ratio of at least 4.5:1.
- **Large Text (18pt and above):** Contrast ratio must be at least 3.0:1.
- **Non-text Contrast:** Interactive controls, icons, and focus rings must maintain at least a 3.0:1 contrast ratio.

### Keyboard & Focus Behaviors
- **Focus Order:** Tab navigation order must flow from left to right, top to bottom:
  1. File input path configs.
  2. Calibration variables and pick target switches.
  3. Interactive viewport (camera key controls).
  4. Point editor inputs.
  5. Action buttons (Load, Bake, Cancel, Save).
- **Focus Indicator:** All focusable elements must display a distinct visual outline when focused (`focus-visible`).
- **Fail Check:** Do not use `outline: none` or `outline: 0` without implementing a custom focus style.

---

## Content and Tone Standards
- **Tone:** Concise, confident, implementation-focused.
- **Language Rules:**
  - The application must use English-only terminology for GUI layout elements to avoid awkward translations of 3D computer graphics keywords (e.g., "Voxelize", "NavMesh", "3D Gaussian Splatting").
  - Always write actions as active verbs: use "Load Source", "Bake Geometry", "Cancel Job", and "Save ZIP".
  - Refrain from using passive voice or ambiguous labels.

---

## Anti-Patterns and Prohibited Implementations
- **Strict Prohibition:** Do not introduce one-off spacing parameters, layout margins, or custom font sizes.
- **Strict Prohibition:** Do not use absolute positioning that overflows containers on viewport sizes down to `320px`.
- **Strict Prohibition:** Do not allow low-contrast elements (contrast < 4.5:1) for secondary descriptions or help text.
- **Strict Prohibition:** Do not deploy forms without accessible `label` tags linked via `for`/`id` bindings.

---

## Naming Conventions

To ensure consistency and ease of maintenance across the GUI codebase, teams must adhere to the following casing rules:

### 1. File and Directory Naming
- **React Components / TypeScript Classes:** Must use `PascalCase` naming (e.g., `Preview.tsx`, `PlayCanvasViewer.ts`, `CameraController.ts`).
- **Utility / Helper Files:** Must use `camelCase` naming (e.g., `recipe.ts`, `calibration.ts`, `splatPreview.ts`).
- **Stylesheets / Asset Files:** Must use `kebab-case` naming (e.g., `styles.css`).
- **Directories:** Must use lowercase or `kebab-case` names (e.g., `domains/calibration`, `domains/viewer`).

### 2. Code Variable & Casing Standards
- **Variables & Functions:** Must use `camelCase` naming (e.g., `isFinitePoint`, `pointDistance`, `handlePointerDown`).
- **Constants:** Must use `UPPER_SNAKE_CASE` or `camelCase` for static configuration objects (e.g., `defaultConfig`, `STAGE_PROGRESS`).
- **Interfaces, Types, and Enums:** Must use `PascalCase` naming (e.g., `SourceMetadata`, `Bounds`, `PickMode`).
- **Tauri Backend Commands (IPC):** Must map to `snake_case` in alignment with backend Rust commands (e.g., `load_source`, `process_job`).

### 3. CSS Class Naming
- **Custom Classes:** Must use simple, lowercase `kebab-case` names (e.g., `.viewport-legend`, `.legend-item`, `.selector-field`).
- **Component Styling Prefix:** Custom styling classes should not overlap or conflict with the `astryx-` prefix utilized by core library styles.

---

## Quality Assurance (QA) Checklist

Before merging component code, the team must verify:
- [ ] Contrast ratio on all text and form fields is verified to be $\ge 4.5:1$.
- [ ] Keyboard navigation tests verify that all buttons, inputs, links, and cards are accessible via `Tab` and triggers execute via `Enter`/`Space`.
- [ ] Custom focus outlines are visible, high-contrast, and comply with `:focus-visible`.
- [ ] The app shell functions down to a width of `320px` without text clipping or layout overlap.
- [ ] All spacing and typography parameters reference the semantic design tokens exactly.
- [ ] Active and hover animations use duration values matching the motion duration tokens.
- [ ] No hardcoded color codes exist in the CSS stylesheet files.
