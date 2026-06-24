"""Artifact manifest schema for ga3d processing jobs."""

from typing import Literal, TypeAlias

from pydantic import BaseModel, ConfigDict, Field


Vec3: TypeAlias = tuple[float, float, float]


class ManifestModel(BaseModel):
    model_config = ConfigDict(extra="forbid", populate_by_name=True)


class SourceInfo(ManifestModel):
    format: Literal["ply", "splat", "sog"]
    splat_count: int = Field(alias="splatCount", ge=0)


class CoordinateSystem(ManifestModel):
    up_axis: Literal["Y", "Z"] = Field(default="Y", alias="upAxis")
    unit: Literal["meter"] = "meter"
    scale: float = Field(default=1.0, gt=0.0)


class BoundsInfo(ManifestModel):
    min: Vec3
    max: Vec3


class ArtifactPaths(ManifestModel):
    scene: str = "scene.glb"
    occlusion: str = "occlusion.glb"
    navmesh: str = "navmesh.glb"
    navmesh_json: str = Field(default="navmesh.json", alias="navmeshJson")


class Metrics(ManifestModel):
    duration_ms: float = Field(default=0.0, alias="durationMs", ge=0.0)
    peak_memory_mb: float = Field(default=0.0, alias="peakMemoryMb", ge=0.0)
    input_splat_count: int = Field(default=0, alias="inputSplatCount", ge=0)
    baked_splat_count: int = Field(default=0, alias="bakedSplatCount", ge=0)
    filtered_splat_count: int = Field(default=0, alias="filteredSplatCount", ge=0)
    occupied_voxel_count: int = Field(default=0, alias="occupiedVoxelCount", ge=0)
    occlusion_triangle_count: int = Field(default=0, alias="occlusionTriangleCount", ge=0)
    navmesh_triangle_count: int = Field(default=0, alias="navmeshTriangleCount", ge=0)


class ArtifactManifest(ManifestModel):
    version: Literal[1] = 1
    source: SourceInfo
    coordinate_system: CoordinateSystem = Field(default_factory=CoordinateSystem, alias="coordinateSystem")
    bounds: BoundsInfo
    artifacts: ArtifactPaths = Field(default_factory=ArtifactPaths)
    metrics: Metrics = Field(default_factory=Metrics)


def make_manifest(
    *,
    source_format: Literal["ply", "splat", "sog"],
    splat_count: int,
    bounds_min: Vec3,
    bounds_max: Vec3,
    artifacts: ArtifactPaths | None = None,
    metrics: Metrics | None = None,
) -> ArtifactManifest:
    return ArtifactManifest(
        source=SourceInfo(format=source_format, splat_count=splat_count),
        bounds=BoundsInfo(min=bounds_min, max=bounds_max),
        artifacts=artifacts or ArtifactPaths(),
        metrics=metrics or Metrics(),
    )
