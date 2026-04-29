# Lumina Scene Format (LSF) Specification

LSF is the core of the Lumina ecosystem. It is designed to be declarative, self-describing, and AI-friendly.

## Design Principles
1. **Declarative only**: No functions, loops, or conditionals. Pure data.
2. **Self-describing**: Every object declares its type and properties.
3. **Validatable**: A published JSON Schema (draft-07) ensures AI agents can validate scenes before submission.

## Full LSF Example

```json
{
  "version": "1.0",
  "meta": {
    "title": "Pythagorean Theorem Proof",
    "author": "lumina-ai-agent",
    "created_at": "2025-06-01T12:00:00Z"
  },
  "canvas": {
    "width": 1920,
    "height": 1080,
    "fps": 60,
    "duration": 12.0,
    "background": "#0F0F1A"
  },
  "objects": {
    "triangle": {
      "type": "Polygon",
      "z_index": 1,
      "properties": {
        "points": [[0, 0], [300, 0], [0, 400]],
        "fill": "#1E3A5F",
        "stroke": "#4A90D9",
        "stroke_width": 2,
        "opacity": 0
      }
    }
  },
  "timeline": [
    {
      "time": 1.0,
      "object": "triangle",
      "state": { "opacity": 1.0 },
      "easing": "spring",
      "easing_params": { "stiffness": 200, "damping": 20 }
    }
  ]
}
```

## Conflict Resolution Rules
- **Rule 1 (Collision)**: Last declaration in the timeline array wins for same object/time/property.
- **Rule 2 (Transforms)**: Child transforms are relative to the parent `Group` transform.
- **Rule 3 (Missing Props)**: Properties not defined at t=0 default to 0 or 1 based on type (opacity defaults to 1.0).
