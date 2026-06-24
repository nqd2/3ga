"""Typer command line interface for ga3d."""

from pathlib import Path
from typing import Optional

import typer

from .jobs import benchmark as run_benchmark
from .jobs import process as run_process


app = typer.Typer(no_args_is_help=True)


@app.callback()
def main() -> None:
    """ga3d command line interface."""


@app.command()
def process(
    input_path: Path = typer.Argument(..., exists=True, file_okay=True, dir_okay=False),
    config: Optional[Path] = typer.Option(None, "--config", exists=True, file_okay=True, dir_okay=False),
    edits: Optional[Path] = typer.Option(None, "--edits", exists=True, file_okay=True, dir_okay=False),
    out: Path = typer.Option(..., "--out", file_okay=False, dir_okay=True),
) -> None:
    result = run_process(input_path=input_path, output_dir=out, config_path=config, edit_recipe_path=edits)
    typer.echo(result.model_dump_json())


@app.command()
def benchmark(
    splats: int = typer.Option(..., "--splats", min=1),
    out: Path = typer.Option(..., "--out", file_okay=False, dir_okay=True),
) -> None:
    report = run_benchmark(splats=splats, output_dir=out)
    typer.echo(report.model_dump_json())


if __name__ == "__main__":
    app()
