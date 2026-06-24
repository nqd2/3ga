"""FastAPI application for ga3d processing jobs."""

import os
from pathlib import Path
from typing import Any

from fastapi import FastAPI, HTTPException
from fastapi.responses import FileResponse
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel, ConfigDict

from .jobs import JobRecord, JobStore


class JobCreateRequest(BaseModel):
    model_config = ConfigDict(extra="forbid")

    input_path: str
    output_dir: str
    config_path: str | None = None
    edit_recipe_path: str | None = None
    edit_recipe: dict[str, Any] | None = None


class JobResponse(BaseModel):
    id: str
    state: str
    states: list[str]
    artifacts: dict[str, str]
    error: str | None = None


def _response(job: JobRecord) -> JobResponse:
    return JobResponse(
        id=job.id,
        state=job.state,
        states=job.states,
        artifacts=job.artifacts,
        error=job.error,
    )


store = JobStore()
app = FastAPI(title="ga3d API")


@app.post("/api/jobs", response_model=JobResponse)
def create_job(request: JobCreateRequest) -> JobResponse:
    job = store.create(
        input_path=request.input_path,
        output_dir=request.output_dir,
        config_path=request.config_path,
        edit_recipe_path=request.edit_recipe_path,
        edit_recipe=request.edit_recipe,
    )
    return _response(job)


@app.get("/api/jobs/{job_id}", response_model=JobResponse)
def get_job(job_id: str) -> JobResponse:
    job = store.get(job_id)
    if job is None:
        raise HTTPException(status_code=404, detail="job not found")
    return _response(job)


@app.get("/api/jobs/{job_id}/artifacts/{name}")
def get_artifact(job_id: str, name: str) -> FileResponse:
    path = store.artifact_path(job_id, name)
    if path is None:
        raise HTTPException(status_code=404, detail="artifact not found")
    if not Path(path).exists():
        raise HTTPException(status_code=404, detail="artifact missing on disk")
    return FileResponse(path)


static_dir = os.getenv("GA3D_STATIC_DIR")
if static_dir and Path(static_dir).exists():
    app.mount("/", StaticFiles(directory=static_dir, html=True), name="web")
