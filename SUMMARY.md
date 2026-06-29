# Project augmented-gaussian Initialization Summary

This document summarizes the requirements, technical design choices, and initial project alignment for the `augmented-gaussian` project.

---

## 1. Project Goal & Scope
The project develops a tool to process 3D Gaussian Splatting (3DGS) data for Augmented Reality (AR) systems. It extracts structured 3D geometry from unstructured 3DGS data to enable virtual objects to interact with physical obstacles and terrain.

---

## 2. Technical Stack & Decisions

### Core Tech Stack
*   **App Wrapper**: Tauri App.
*   **Backend (Rust)**: Handles heavy file parsing, coordinate transformations, and data packaging.
*   **GPU Compute (Rust)**: `wgpu` crate for cross-platform GPU-accelerated voxelization and carving.
*   **Frontend (React + Vite + TypeScript)**: Standard web view frontend inside Tauri.
*   **3D Rendering**: PlayCanvas Engine in the Webview for native 3DGS loading, interactive preview, and gizmos.
*   **NavMesh Generation**: Pure Rust `rerecast` wrapper in `core-lib`, baking navmeshes from the post fill/carve collision mesh.

---

## 3. Data Processing Pipeline & Logic

### Input Format
Supports `.ply`, `.splat`, `.sog`, and `meta.json`.

### Data Model (Structure of Arrays)
Stores splats in columnar vectors rather than heap objects:
*   Positions: `x`, `y`, `z`
*   Scales: `scale_0`, `scale_1`, `scale_2` (stored as log scale `ln(s)`)
*   Opacity: `opacity` (stored as logit scale `ln(alpha / (1 - alpha))`)
*   Spherical Harmonics: `f_dc_0..2` (color) and optionally `f_rest_0..44`
*   Rotation: `rot_0..3` (normalized quaternion)

### Geometry & Voxelization Pipeline
1.  **Recipe-Based Edits**: Edits are stored as a JSON recipe stack on the frontend. The backend replays this recipe on the original file to bake transforms and filter data.
2.  **Voxel Grid Alignment**: Grid bounds are aligned to 4x4x4 block boundaries (`block_size = 4 * voxel_resolution`).
3.  **Voxelization**: Opacity is voxelized through the CPU reference path or `wgpu`, with CPU/GPU parity checks available.
4.  **Fill**: `exterior-fill` dilates and boundary-flood-fills closed indoor volumes; `floor-fill` fills each XZ column from bottom to first solid voxel for terrain/outdoor scenes.
5.  **Capsule Carving**: A capsule flood-fill from `seed_pos` keeps only solid voxels bordering reachable agent space, using configured agent height and radius.
6.  **Collision Mesh Extraction**: `faces` emits exposed voxel faces; `smooth` runs Marching Cubes followed by coplanar face merge.
7.  **Navmesh Baking**: `rerecast` runs in Rust on the post fill/carve triangle mesh and outputs `navmesh.glb` plus `navmesh.bin` only when walkable polygons exist.

---

## 4. Export Artifact Bundle
The tool exports a single WebAR ZIP bundle containing:
*   `index.html`: Standalone click-and-run PlayCanvas visualizer.
*   `assets/js/playcanvas.min.js`: Local PlayCanvas runtime for the viewer.
*   `scene.sog`: Processed SOG V2 splat bundle.
*   `occlusion.glb`: Reconstructed occlusion/collision geometry.
*   `navmesh.glb` or `navmesh.bin`: Optional generated navigation mesh.
*   `manifest.json`: Metadata defining calibrated transform, unit scale, bounds, processing parameters, metrics, and file-size ratios.

---

## 5. Development Guidelines
We established the developer instructions in `AGENTS.md` covering:
*   **Karpathy Guidelines**: Simplicity, surgical edits, explicit assumptions, and goal-driven execution.
*   **Stop-Slop Rules**: Direct prose, active voice, specific statements, and no AI slop.
*   **Caveman Mode**: Terse, compressed English responses to optimize context.
*   **Writing Plans & Brainstorming**: Strict plan layouts, test-first patterns, and design approval gates before code implementation.
