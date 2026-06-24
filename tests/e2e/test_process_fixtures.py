from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "python"))

from ga3d.jobs import benchmark, process  # noqa: E402


def _assert_glb(path: Path) -> None:
    assert path.exists(), path
    assert path.read_bytes()[:4] == b"glTF"


def _run_case(input_path: Path, out_dir: Path) -> None:
    edits = out_dir.parent / f"{input_path.stem}-edits.json"
    edits.write_text(
        json.dumps(
            {
                "version": 1,
                "operations": [
                    {"type": "selectAll"},
                    {
                        "type": "transformSelected",
                        "matrix": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0.25, 0, 0, 1],
                    },
                ],
            }
        )
    )
    result = process(input_path=input_path, output_dir=out_dir, edit_recipe_path=edits)
    assert result.states[-1] == "done"
    for name in ["scene.glb", "occlusion.glb", "navmesh.glb"]:
        _assert_glb(out_dir / name)
    navmesh = json.loads((out_dir / "navmesh.json").read_text())
    manifest = json.loads((out_dir / "manifest.json").read_text())
    assert navmesh["coordinateSystem"] == {"upAxis": "Y", "unit": "meter"}
    assert navmesh["indices"]
    assert manifest["source"]["splatCount"] > 0
    assert manifest["bounds"]["min"][0] > -1.0
    assert manifest["artifacts"] == {
        "scene": "scene.glb",
        "occlusion": "occlusion.glb",
        "navmesh": "navmesh.glb",
        "navmeshJson": "navmesh.json",
    }
    assert manifest["metrics"]["inputSplatCount"] >= manifest["metrics"]["filteredSplatCount"] > 0
    assert manifest["metrics"]["occlusionTriangleCount"] > 1
    assert manifest["metrics"]["navmeshTriangleCount"] > 1


def test_process_fixtures() -> None:
    with tempfile.TemporaryDirectory() as raw_tmp:
        tmp = Path(raw_tmp)
        _run_case(ROOT / "tests/fixtures/minimal.ply", tmp / "ply")
        _run_case(ROOT / "tests/fixtures/minimal.splat", tmp / "splat")

        for sog in sorted((ROOT / "tests/fixtures").glob("*.sog")):
            _run_case(sog, tmp / sog.stem)

        report = benchmark(splats=5_000, output_dir=tmp / "bench")
        data = report.model_dump()
        for key in [
            "splatCount",
            "processedSplatCount",
            "peakMemoryMb",
            "durationMs",
            "sceneGlbMb",
            "occlusionTriangleCount",
            "navmeshTriangleCount",
        ]:
            assert key in data
        assert data["occlusionTriangleCount"] > 1
        assert data["navmeshTriangleCount"] > 1


if __name__ == "__main__":
    test_process_fixtures()
