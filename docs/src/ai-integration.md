# AI Integration Cookbook

Because LSF is declarative data, an LLM can author a scene directly, a validator
can check it, and the engine renders it deterministically. The validator returns
structured errors with `fix_suggestion` strings designed to be re-injected for
self-correction.

## The generate → validate → render loop (Python)

```python
import json, luminafx, anthropic

client = anthropic.Anthropic()

SYSTEM = """You generate Lumina Scene Format (LSF) JSON.
- Objects go in "objects" with a "type" and "properties".
- Timeline entries: time (float), object (id), state (object), easing (string).
- Return ONLY JSON."""

def ask(content: str) -> dict:
    msg = client.beta.messages.create(
        model="claude-opus-5",
        max_tokens=16000,
        system=SYSTEM,
        # A declined request is re-run server-side on the recommended fallback
        # model instead of coming back as a refusal.
        betas=["server-side-fallback-2026-07-01"],
        fallbacks="default",
        messages=[{"role": "user", "content": content}],
    )
    if msg.stop_reason in ("refusal", "max_tokens"):
        raise RuntimeError(f"no scene: stop_reason={msg.stop_reason}")
    # The reply can open with a thinking block, so read the text blocks rather
    # than assuming the first block is text.
    return json.loads("".join(b.text for b in msg.content if b.type == "text"))

scene = ask("Explain the dot product of two vectors in 10 seconds.")
report = luminafx.validate(scene)
while not report["valid"]:
    feedback = "\n".join(f"{e['code']}: {e['message']} → {e['fix_suggestion']}" for e in report["errors"])
    scene = ask("Fix this scene. Errors:\n" + feedback + "\n\nScene:\n" + json.dumps(scene))
    report = luminafx.validate(scene)

luminafx.render(scene, "explainer.mp4", format="mp4")
```

## Let the engine fix what needs no judgement

Many validation errors have exactly one sensible repair: `"raduis"` next to a
real `radius`, a timeline entry naming `"circel"` when `"circle"` exists, an
easing called `ease_out_cubicc`. Sending those back to a model costs a round
trip and a chance to get them wrong again. The validator attaches the repair to
such errors as a `fix_patch`, an RFC 6902 JSON Patch against the scene:

```json
{
  "code": "UNKNOWN_PROPERTY",
  "path": "$.timeline[0].state.raduis",
  "message": "\"raduis\" is not a property of Circle.",
  "fix_suggestion": "Did you mean 'radius'?",
  "fix_patch": [{ "op": "move", "from": "/timeline/0/state/raduis",
                  "path": "/timeline/0/state/radius" }]
}
```

Only certain repairs get a patch — a misspelled property name, object id, object
type, asset id, axes id, group child or easing name with a single near match. A
wrong type, a value out of range, or a name with nothing close stays prose,
because it needs a decision.

Three ways to apply them, each of which re-validates and repeats until nothing
more can be fixed this way (one repair can expose the next):

```bash
lumina-cli fix scene.lsf            # show what would change
lumina-cli fix scene.lsf --write    # change only the misspelled words, in place
lumina-cli fix scene.lsf --json     # the fixes, the repaired scene, and what is left
```

- **MCP:** the `lumina_fix` tool takes a scene and returns the repaired scene,
  each fix applied, and the remaining validation.
- **HTTP:** send an error's `fix_patch` to `POST /patch` with the scene.

Then hand the model only what is left in `remaining`: the errors that need it.

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

- Inject `luminafx.schema()` (or `/schema`) into the system prompt so the model grounds property names.
- Tell the model: object IDs are snake_case; the timeline is sorted by `time`; colors are hex; group children use coordinates relative to the group.
- Use `/objects` to give the model a compact "required vs optional" cheat sheet instead of the full schema when context is tight.
