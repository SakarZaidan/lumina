"""The Python SDK against the contract every other entry point keeps.

`lumina-cli`, the HTTP server and the MCP tools all validate from the raw
document, refuse to render what does not validate, and repair what needs no
judgement. These check that `luminafx` does the same.
"""

import copy

import pytest

import luminafx


def scene():
    return {
        "version": "1.0",
        "meta": {"title": "t", "author": "a", "created_at": "2026-01-01T00:00:00Z"},
        "canvas": {"width": 64, "height": 64, "fps": 30, "duration": 1.0,
                   "background": "#000000"},
        "objects": {
            "dot": {"type": "Circle", "properties": {"cx": 32, "cy": 32, "radius": 10}},
        },
        "timeline": [],
    }


def misspelled():
    s = scene()
    s["objects"]["dot"]["properties"]["opacty"] = 0.5
    return s


def test_a_valid_scene_validates():
    report = luminafx.validate(scene())
    assert report["valid"], report["errors"]


def test_a_misspelled_property_is_reported_with_its_repair():
    # Parsing drops a property it does not know, so this came back valid
    # before the SDK validated the dict itself.
    report = luminafx.validate(misspelled())
    assert not report["valid"]
    error = report["errors"][0]
    assert error["code"] == "UNKNOWN_PROPERTY"
    assert "opacity" in error["fix_suggestion"]
    assert error["fix_patch"][0]["path"] == "/objects/dot/properties/opacity"


def test_something_that_is_not_json_is_a_failed_validation_not_a_crash():
    report = luminafx.validate({"objects": {1, 2, 3}})
    assert not report["valid"]
    assert report["errors"][0]["code"] == "PARSE_ERROR"


def test_fix_repairs_what_needs_no_judgement_and_leaves_the_input_alone():
    original = misspelled()
    untouched = copy.deepcopy(original)
    result = luminafx.fix(original)
    assert original == untouched
    assert result["remaining"]["valid"], result["remaining"]["errors"]
    assert result["scene"]["objects"]["dot"]["properties"]["opacity"] == 0.5
    assert [fix["code"] for fix in result["applied"]] == ["UNKNOWN_PROPERTY"]


def test_render_refuses_what_validate_rejects_before_rendering(tmp_path):
    with pytest.raises(ValueError, match="UNKNOWN_PROPERTY"):
        luminafx.render(misspelled(), str(tmp_path / "out.mp4"))
    assert not (tmp_path / "out.mp4").exists()


def test_the_schema_is_a_json_schema():
    schema = luminafx.schema()
    assert "definitions" in schema or "$defs" in schema
