# AI Integration Cookbook

Because LSF is declarative data, an LLM can author a scene directly, a validator
can check it, and the engine renders it deterministically. The validator returns
structured errors with `fix_suggestion` strings designed to be re-injected for
self-correction.

## The generate → validate → render loop (Python)

```python
import json, lumina, anthropic

client = anthropic.Anthropic()

SYSTEM = """You generate Lumina Scene Format (LSF) JSON.
- Objects go in "objects" with a "type" and "properties".
- Timeline entries: time (float), object (id), state (object), easing (string).
- Return ONLY JSON."""

msg = client.messages.create(
    model="claude-sonnet-4-6", max_tokens=4096, system=SYSTEM,
    messages=[{"role": "user", "content": "Explain the dot product of two vectors in 10 seconds."}],
)
scene = json.loads(msg.content[0].text)

report = lumina.validate(scene)
while not report["valid"]:
    feedback = "\n".join(f"{e['code']}: {e['message']} → {e['fix_suggestion']}" for e in report["errors"])
    msg = client.messages.create(
        model="claude-sonnet-4-6", max_tokens=4096, system=SYSTEM,
        messages=[
            {"role": "user", "content": "Fix this scene. Errors:\n" + feedback + "\n\nScene:\n" + json.dumps(scene)},
        ],
    )
    scene = json.loads(msg.content[0].text)
    report = lumina.validate(scene)

lumina.render(scene, "explainer.mp4", format="mp4")
```

## Over HTTP

```bash
# Pre-validate before spending render time
curl -X POST localhost:3000/validate -H 'Content-Type: application/json' -d @scene.json

# Discover the object registry (required/optional props per type)
curl localhost:3000/objects | jq

# Fetch the live JSON Schema for prompt-time grounding / IDE autocomplete
curl localhost:3000/schema | jq '.title'
```

## Prompting tips

- Inject `lumina.schema()` (or `/schema`) into the system prompt so the model grounds property names.
- Tell the model: object IDs are snake_case; the timeline is sorted by `time`; colors are hex; group children use coordinates relative to the group.
- Use `/objects` to give the model a compact "required vs optional" cheat sheet instead of the full schema when context is tight.
