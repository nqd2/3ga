"""Pydantic schemas for ga3d API payloads."""

from typing import Annotated, Literal, TypeAlias, Union

from pydantic import BaseModel, ConfigDict, Field, field_validator


Vec3: TypeAlias = tuple[float, float, float]
Matrix4: TypeAlias = tuple[float, ...]


class RecipeModel(BaseModel):
    model_config = ConfigDict(extra="forbid")


class SelectAllOperation(RecipeModel):
    type: Literal["selectAll"]


class SelectNoneOperation(RecipeModel):
    type: Literal["selectNone"]


class SelectBoxOperation(RecipeModel):
    type: Literal["selectBox"]
    mode: Literal["set", "add", "remove"] = "set"
    center: Vec3
    size: Vec3

    @field_validator("size")
    @classmethod
    def size_must_be_positive(cls, value: Vec3) -> Vec3:
        if any(component <= 0 for component in value):
            raise ValueError("selectBox size components must be positive")
        return value


class DeleteSelectedOperation(RecipeModel):
    type: Literal["deleteSelected"]


class TransformSelectedOperation(RecipeModel):
    type: Literal["transformSelected"]
    matrix: Matrix4

    @field_validator("matrix")
    @classmethod
    def matrix_must_have_16_values(cls, value: Matrix4) -> Matrix4:
        if len(value) != 16:
            raise ValueError("transformSelected matrix must contain 16 numbers")
        return value


class FilterOpacityOperation(RecipeModel):
    type: Literal["filterOpacity"]
    min: float = Field(ge=0.0, le=1.0)


EditOperation: TypeAlias = Annotated[
    Union[
        SelectAllOperation,
        SelectNoneOperation,
        SelectBoxOperation,
        DeleteSelectedOperation,
        TransformSelectedOperation,
        FilterOpacityOperation,
    ],
    Field(discriminator="type"),
]


class EditRecipe(RecipeModel):
    version: Literal[1] = 1
    operations: list[EditOperation] = Field(default_factory=list)
