import pytest
from pydantic import ValidationError

from ga3d.manifest import ArtifactManifest, make_manifest


def test_make_manifest_matches_public_shape() -> None:
    manifest = make_manifest(
        source_format="splat",
        splat_count=42,
        bounds_min=(-1.0, 0.0, -2.0),
        bounds_max=(1.0, 2.0, 3.0),
    )

    data = manifest.model_dump(by_alias=True)
    assert data["version"] == 1
    assert data["source"] == {"format": "splat", "splatCount": 42}
    assert data["coordinateSystem"] == {"upAxis": "Y", "unit": "meter", "scale": 1.0}
    assert data["artifacts"]["scene"] == "scene.glb"
    assert data["artifacts"]["navmeshJson"] == "navmesh.json"
    assert data["metrics"] == {
        "durationMs": 0.0,
        "peakMemoryMb": 0.0,
        "inputSplatCount": 0,
        "bakedSplatCount": 0,
        "filteredSplatCount": 0,
        "occupiedVoxelCount": 0,
        "occlusionTriangleCount": 0,
        "navmeshTriangleCount": 0,
    }


def test_manifest_round_trips_from_json_shape() -> None:
    manifest = ArtifactManifest.model_validate(
        {
            "version": 1,
            "source": {"format": "ply", "splatCount": 7},
            "coordinateSystem": {"upAxis": "Y", "unit": "meter", "scale": 1.0},
            "bounds": {"min": [0, 0, 0], "max": [1, 1, 1]},
            "artifacts": {
                "scene": "scene.glb",
                "occlusion": "occlusion.glb",
                "navmesh": "navmesh.glb",
                "navmeshJson": "navmesh.json",
            },
            "metrics": {
                "durationMs": 12.5,
                "peakMemoryMb": 64,
                "inputSplatCount": 7,
                "bakedSplatCount": 7,
                "filteredSplatCount": 6,
                "occupiedVoxelCount": 40,
                "occlusionTriangleCount": 12,
                "navmeshTriangleCount": 8,
            },
        }
    )

    assert manifest.source.splat_count == 7
    assert manifest.metrics.duration_ms == 12.5
    assert manifest.metrics.filtered_splat_count == 6


def test_manifest_rejects_negative_counts() -> None:
    with pytest.raises(ValidationError):
        make_manifest(
            source_format="sog",
            splat_count=-1,
            bounds_min=(0, 0, 0),
            bounds_max=(1, 1, 1),
        )
