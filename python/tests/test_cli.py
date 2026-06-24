import json
from pathlib import Path

from typer.testing import CliRunner

from ga3d.cli import app


ROOT = Path(__file__).resolve().parents[2]


def test_cli_process_creates_artifacts(tmp_path) -> None:
    edits_path = tmp_path / "edits.json"
    edits_path.write_text(
        json.dumps(
            {
                "version": 1,
                "operations": [
                    {"type": "selectAll"},
                    {
                        "type": "transformSelected",
                        "matrix": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0.5, 0, 0, 1],
                    },
                ],
            }
        )
    )
    out_dir = tmp_path / "out"

    result = CliRunner().invoke(
        app,
        ["process", str(ROOT / "tests/fixtures/minimal.splat"), "--edits", str(edits_path), "--out", str(out_dir)],
    )

    assert result.exit_code == 0, result.output
    payload = json.loads(result.output)
    assert payload["states"][-1] == "done"
    for name in ["scene.glb", "occlusion.glb", "navmesh.glb", "navmesh.json", "manifest.json"]:
        assert (out_dir / name).exists()
    manifest = json.loads((out_dir / "manifest.json").read_text())
    assert manifest["source"] == {"format": "splat", "splatCount": 4}
    assert manifest["bounds"]["min"][0] == -0.5
    assert manifest["metrics"]["occlusionTriangleCount"] > 1
    assert manifest["metrics"]["navmeshTriangleCount"] > 1


def test_cli_benchmark_writes_report(tmp_path) -> None:
    out_dir = tmp_path / "bench"
    result = CliRunner().invoke(app, ["benchmark", "--splats", "5000", "--out", str(out_dir)])

    assert result.exit_code == 0, result.output
    payload = json.loads(result.output)
    assert payload["splatCount"] == 5000
    assert payload["processedSplatCount"] == 5000
    assert payload["occlusionTriangleCount"] > 1
    assert payload["navmeshTriangleCount"] > 1
    assert (out_dir / "benchmark.json").exists()
