import pytest
from pydantic import ValidationError

from ga3d.schemas import EditRecipe, SelectBoxOperation, TransformSelectedOperation


def test_edit_recipe_accepts_supported_operations() -> None:
    recipe = EditRecipe.model_validate(
        {
            "version": 1,
            "operations": [
                {"type": "selectAll"},
                {"type": "selectNone"},
                {"type": "selectBox", "mode": "set", "center": [0, 0, 0], "size": [1, 2, 3]},
                {"type": "deleteSelected"},
                {"type": "transformSelected", "matrix": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 4, 5, 6, 1]},
                {"type": "filterOpacity", "min": 0.05},
            ],
        }
    )

    assert recipe.version == 1
    assert len(recipe.operations) == 6
    assert isinstance(recipe.operations[2], SelectBoxOperation)
    assert isinstance(recipe.operations[4], TransformSelectedOperation)


def test_transform_matrix_requires_16_values() -> None:
    with pytest.raises(ValidationError):
        EditRecipe.model_validate(
            {
                "version": 1,
                "operations": [
                    {"type": "transformSelected", "matrix": [1, 0, 0]},
                ],
            }
        )


def test_filter_opacity_range_is_validated() -> None:
    with pytest.raises(ValidationError):
        EditRecipe.model_validate(
            {
                "version": 1,
                "operations": [
                    {"type": "filterOpacity", "min": 1.5},
                ],
            }
        )


def test_select_box_size_must_be_positive() -> None:
    with pytest.raises(ValidationError):
        EditRecipe.model_validate(
            {
                "version": 1,
                "operations": [
                    {"type": "selectBox", "center": [0, 0, 0], "size": [1, 0, 1]},
                ],
            }
        )
