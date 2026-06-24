from __future__ import annotations

from pathlib import Path

from pybind11.setup_helpers import Pybind11Extension, build_ext
from setuptools import setup


ROOT = Path(__file__).resolve().parent.parent
CORE = ROOT / "core"
CORE_REL = Path("..") / "core"


extension = Pybind11Extension(
    "ga3d._ga3d_core",
    [
        "ga3d/bindings.cpp",
        str(CORE_REL / "src/collision_mesh.cpp"),
        str(CORE_REL / "src/data_table.cpp"),
        str(CORE_REL / "src/edit_recipe.cpp"),
        str(CORE_REL / "src/gaussian_extents.cpp"),
        str(CORE_REL / "src/glb_writer.cpp"),
        str(CORE_REL / "src/navmesh.cpp"),
        str(CORE_REL / "src/read_ply.cpp"),
        str(CORE_REL / "src/read_sog.cpp"),
        str(CORE_REL / "src/read_splat.cpp"),
        str(CORE_REL / "src/voxel_grid.cpp"),
        str(CORE_REL / "src/voxelize.cpp"),
    ],
    cxx_std=20,
    include_dirs=[str(CORE / "include")],
    libraries=["z", "webp"],
)


setup(
    ext_modules=[extension],
    cmdclass={"build_ext": build_ext},
)
