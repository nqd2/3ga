"""Synchronous processing jobs for the ga3d API and CLI."""

from __future__ import annotations

import json
import time
import uuid
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Literal

from pydantic import BaseModel, ConfigDict

from . import _ga3d_core
from .manifest import make_manifest
from .schemas import EditRecipe


JobState = Literal[
    "queued",
    "reading",
    "editing",
    "filtering",
    "voxelizing",
    "meshing",
    "navmesh",
    "exporting",
    "done",
    "failed",
]


ARTIFACT_NAMES = {
    "scene": "scene.glb",
    "occlusion": "occlusion.glb",
    "navmesh": "navmesh.glb",
    "navmeshJson": "navmesh.json",
    "manifest": "manifest.json",
}


class ProcessResult(BaseModel):
    model_config = ConfigDict(extra="forbid")

    output_dir: str
    artifacts: dict[str, str]
    states: list[JobState]


class BenchmarkReport(BaseModel):
    splatCount: int
    processedSplatCount: int
    peakMemoryMb: float
    durationMs: float
    sceneGlbMb: float
    occlusionTriangleCount: int
    navmeshTriangleCount: int


@dataclass
class JobRecord:
    id: str
    input_path: Path
    output_dir: Path
    config_path: Path | None = None
    edit_recipe_path: Path | None = None
    edit_recipe: dict[str, Any] | None = None
    state: JobState = "queued"
    states: list[JobState] = field(default_factory=lambda: ["queued"])
    artifacts: dict[str, str] = field(default_factory=dict)
    error: str | None = None

    def set_state(self, state: JobState) -> None:
        self.state = state
        self.states.append(state)


def _source_format(path: Path) -> Literal["ply", "splat", "sog"]:
    suffix = path.suffix.lower().lstrip(".")
    if suffix in {"ply", "splat", "sog"}:
        return suffix  # type: ignore[return-value]
    raise ValueError(f"unsupported input format: {path.suffix}")


def _read_edit_recipe(path: Path | None) -> EditRecipe:
    if path is None:
        return EditRecipe()
    return EditRecipe.model_validate_json(path.read_text())


def _read_config(path: Path | None) -> dict[str, Any]:
    if path is None:
        return {}
    raw = path.read_text().strip()
    if not raw:
        return {}
    try:
        parsed = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise ValueError(f"config must be JSON: {path}") from exc
    if not isinstance(parsed, dict):
        raise ValueError("config root must be an object")
    return parsed


def _recipe_payload(path: Path | None, recipe: dict[str, Any] | EditRecipe | None) -> dict[str, Any]:
    if path is not None:
        return _read_edit_recipe(path).model_dump(mode="json")
    if recipe is None:
        return EditRecipe().model_dump(mode="json")
    if isinstance(recipe, EditRecipe):
        return recipe.model_dump(mode="json")
    return EditRecipe.model_validate(recipe).model_dump(mode="json")


def _bounds_tuple(summary: dict[str, Any], key: str) -> tuple[float, float, float]:
    values = summary["bounds"][key]
    return (float(values[0]), float(values[1]), float(values[2]))


def process(
    *,
    input_path: str | Path,
    output_dir: str | Path,
    config_path: str | Path | None = None,
    edit_recipe_path: str | Path | None = None,
    edit_recipe: dict[str, Any] | EditRecipe | None = None,
) -> ProcessResult:
    input_file = Path(input_path)
    out_dir = Path(output_dir)
    config_file = Path(config_path) if config_path else None
    edits_file = Path(edit_recipe_path) if edit_recipe_path else None

    states: list[JobState] = ["queued"]
    start = time.perf_counter()
    if not input_file.exists():
        raise FileNotFoundError(input_file)
    if config_file is not None and not config_file.exists():
        raise FileNotFoundError(config_file)

    states.append("reading")
    source_format = _source_format(input_file)
    out_dir.mkdir(parents=True, exist_ok=True)

    states.append("editing")
    recipe = _recipe_payload(edits_file, edit_recipe)

    states.extend(["filtering", "voxelizing", "meshing", "navmesh", "exporting"])
    pipeline = _ga3d_core.run_pipeline(input_file, out_dir, recipe, _read_config(config_file))
    filtered = pipeline["filtered"]
    grid = pipeline["grid"]
    occlusion_mesh = pipeline["occlusionMesh"]
    navmesh = pipeline["navmesh"]

    manifest = make_manifest(
        source_format=source_format,
        splat_count=int(pipeline["input"]["splatCount"]),
        bounds_min=_bounds_tuple(filtered, "min"),
        bounds_max=_bounds_tuple(filtered, "max"),
    )
    manifest.metrics.duration_ms = (time.perf_counter() - start) * 1000.0
    manifest.metrics.input_splat_count = int(pipeline["input"]["splatCount"])
    manifest.metrics.baked_splat_count = int(pipeline["edited"]["splatCount"])
    manifest.metrics.filtered_splat_count = int(filtered["splatCount"])
    manifest.metrics.occupied_voxel_count = int(grid["occupiedVoxelCount"])
    manifest.metrics.occlusion_triangle_count = int(occlusion_mesh["triangleCount"])
    manifest.metrics.navmesh_triangle_count = int(navmesh["triangleCount"])
    (out_dir / "manifest.json").write_text(manifest.model_dump_json(by_alias=True))

    states.append("done")
    return ProcessResult(
        output_dir=str(out_dir),
        artifacts={name: str(out_dir / filename) for name, filename in ARTIFACT_NAMES.items()},
        states=states,
    )


def benchmark(*, splats: int, output_dir: str | Path) -> BenchmarkReport:
    if splats <= 0:
        raise ValueError("splats must be positive")
    out_dir = Path(output_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    start = time.perf_counter()
    processed_splats = min(splats, 50_000)
    pipeline = _ga3d_core.run_synthetic_pipeline(
        processed_splats,
        out_dir,
        {"voxelSize": 0.08, "opacityCutoff": 0.05, "sigma": 1.5},
    )
    occlusion_mesh = pipeline["occlusionMesh"]
    navmesh = pipeline["navmesh"]
    report = BenchmarkReport(
        splatCount=splats,
        processedSplatCount=processed_splats,
        peakMemoryMb=0.0,
        durationMs=(time.perf_counter() - start) * 1000.0,
        sceneGlbMb=(out_dir / "scene.glb").stat().st_size / (1024.0 * 1024.0),
        occlusionTriangleCount=int(occlusion_mesh["triangleCount"]),
        navmeshTriangleCount=int(navmesh["triangleCount"]),
    )
    (out_dir / "benchmark.json").write_text(report.model_dump_json())
    return report


class JobStore:
    def __init__(self) -> None:
        self._jobs: dict[str, JobRecord] = {}

    def create(
        self,
        *,
        input_path: str | Path,
        output_dir: str | Path,
        config_path: str | Path | None = None,
        edit_recipe_path: str | Path | None = None,
        edit_recipe: dict[str, Any] | None = None,
    ) -> JobRecord:
        job = JobRecord(
            id=str(uuid.uuid4()),
            input_path=Path(input_path),
            output_dir=Path(output_dir),
            config_path=Path(config_path) if config_path else None,
            edit_recipe_path=Path(edit_recipe_path) if edit_recipe_path else None,
            edit_recipe=edit_recipe,
        )
        self._jobs[job.id] = job
        try:
            result = process(
                input_path=job.input_path,
                output_dir=job.output_dir,
                config_path=job.config_path,
                edit_recipe_path=job.edit_recipe_path,
                edit_recipe=job.edit_recipe,
            )
            for state in result.states[1:]:
                job.set_state(state)
            job.artifacts = result.artifacts
        except Exception as exc:  # pragma: no cover - defensive API surface
            job.error = str(exc)
            job.set_state("failed")
        return job

    def get(self, job_id: str) -> JobRecord | None:
        return self._jobs.get(job_id)

    def artifact_path(self, job_id: str, name: str) -> Path | None:
        job = self.get(job_id)
        if job is None:
            return None
        raw = job.artifacts.get(name)
        return Path(raw) if raw else None
