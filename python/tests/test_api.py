from pathlib import Path

from fastapi.testclient import TestClient

from ga3d.api import app


ROOT = Path(__file__).resolve().parents[2]


def test_api_job_lifecycle_and_artifact_download(tmp_path) -> None:
    input_path = ROOT / "tests/fixtures/minimal.ply"
    out_dir = tmp_path / "job"
    client = TestClient(app)

    created = client.post(
        "/api/jobs",
        json={
            "input_path": str(input_path),
            "output_dir": str(out_dir),
            "edit_recipe": {"version": 1, "operations": [{"type": "selectAll"}]},
        },
    )
    assert created.status_code == 200
    payload = created.json()
    assert payload["state"] == "done"
    assert payload["states"][:2] == ["queued", "reading"]

    status = client.get(f"/api/jobs/{payload['id']}")
    assert status.status_code == 200
    assert status.json()["artifacts"]["scene"].endswith("scene.glb")

    artifact = client.get(f"/api/jobs/{payload['id']}/artifacts/scene")
    assert artifact.status_code == 200
    assert artifact.content[:4] == b"glTF"
