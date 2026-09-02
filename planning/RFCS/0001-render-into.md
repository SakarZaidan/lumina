# RFC-0001: `Renderer::render_into` — render without allocating the output

- **Status:** Rejected
- **Author:** Sakar Hashim
- **Created:** 2026-09-02
- **Related:** `AAA-P-02` (plan/02-performance.md), TD-03, PR #66

## Problem

`Renderer::render_frame` returns `Result<Vec<u8>, RendererError>`. The
signature *requires* an allocation and a full copy of the frame on every call:

```rust
Ok(pixmap.data().to_vec())
```

At 1080p that copy is 8.3 MB. Measured in isolation it costs **0.394 ms**, out
of roughly 0.566 ms of fixed per-frame cost once the frame buffer itself is
reused (PR #66). So the copy is about **70% of what remains** after the largest
optimisation of Wave 3, and no caller can avoid it, because the return type
does not let them.

Two callers care.

**Export** renders thousands of frames in sequence and immediately hands the
bytes to ffmpeg or to a PNG encoder. It could reuse one buffer for the entire
run. On a 1 560-frame 1080p export that is 0.6 s of pure copying.

**The WASM player** calls `render_frame` per displayed frame. A browser at
60 fps has 16.7 ms per frame; spending 0.4 ms of it copying, plus the
allocator churn of a fresh 8.3 MB `Vec` every frame, is headroom given away for
nothing.

## Proposal

Add one method to the `Renderer` trait, with a default implementation:

```rust
pub trait Renderer {
    /// Render into a caller-supplied buffer.
    ///
    /// `out` must be exactly `width * height * 4` bytes. Returns an error
    /// rather than resizing, so a caller cannot silently pay for a
    /// reallocation it was trying to avoid.
    fn render_into(
        &mut self,
        objects: &HashMap<String, Object>,
        states: &HashMap<String, Value>,
        width: u32,
        height: u32,
        background: &str,
        camera: Option<&CameraState>,
        out: &mut [u8],
    ) -> Result<(), RendererError> {
        // Default: correct for any implementation, faster for none.
        let frame = self.render_frame(objects, states, width, height, background, camera)?;
        if out.len() != frame.len() {
            return Err(RendererError::Failed(format!(
                "output buffer is {} bytes, expected {}",
                out.len(),
                frame.len()
            )));
        }
        out.copy_from_slice(&frame);
        Ok(())
    }
}
```

Backends override it to write directly into `out`. `render_frame` stays, and
keeps its exact meaning — it becomes a convenience wrapper for callers who want
an owned `Vec`.

**Why a defaulted method rather than changing `render_frame`.** Changing the
existing signature would break every implementor and every caller for a benefit
most of them do not want. A default means third-party `Renderer`
implementations keep compiling and are automatically correct; they simply do
not get the speedup until they override it.

**Why an error rather than resizing.** `out: &mut Vec<u8>` with an internal
`resize` would be friendlier and would defeat the purpose: a caller who passes
a wrong-sized buffer wants to know, not to be quietly charged for the
allocation they were avoiding.

## Alternatives

**Do nothing.** The copy stays at ~70% of the fixed per-frame cost. Acceptable
for offline export, wasteful in a browser. Rejected because the cost is
measured, not speculative.

**Change `render_frame` to take `&mut [u8]`.** Breaks every implementor and
caller. The RFC gate exists partly to stop this kind of change; a defaulted
addition achieves the same result compatibly.

**Return `Cow<[u8]>` or a borrowed slice.** `&[u8]` borrowed from the renderer
would be zero-copy and would immediately conflict with the pipelined export
landed in PR #71, which sends frames to another thread. The borrow would have
to end before the next `render_frame`, which is exactly what a pipeline cannot
promise.

**An internal buffer pool inside the renderer.** Hides the allocation instead
of removing it, and the renderer cannot know how long a caller keeps a frame.

## Trade-offs

- **API surface grows by one method.** It is defaulted, so implementors may
  ignore it, but it is one more thing to document and keep correct.
- **Two paths to the same pixels**, which is a place for them to drift. Both
  backends must produce identical output through either, and the tests below
  assert exactly that.
- **The default implementation is slower than `render_frame` alone** — it
  copies twice. That is the honest cost of compatibility: an implementor who
  does not override it pays a little for the convenience of not having to.

## Migration

None. Purely additive with a default body; no existing code changes behaviour,
no scene is affected, no SDK surface moves. `lumina-export` and `lumina-wasm`
adopt it; anything else continues to work unchanged.

## Performance — and why this RFC was rejected

**Expected:** the ~0.394 ms per-1080p-frame copy removed for callers that reuse
a buffer.

**Measured: no improvement, and a slight regression.** A 1 560-frame 1080p MP4
export went from 9.82 s to 10.52 s with the exporter converted to
`render_into`.

The reason is in this document's own Alternatives section, which rejected a
borrowed slice because "the borrow would have to end before the next
`render_frame`, which is exactly what a pipeline cannot promise." That argument
applies to the proposal too, and it was missed.

**Every current caller needs owned bytes:**

- **Video export** hands each frame to a writer thread through a bounded
  channel (PR #71). A channel needs ownership, so the copy `render_into`
  removes is immediately reintroduced by `to_vec` on the send — one copy
  either way, plus a persistent buffer that now has to be kept alive.
- **PNG export** builds an `ImageBuffer::from_raw`, which consumes a `Vec`.
- **The WASM player** returns `Vec<u8>` across the JavaScript boundary, which
  copies regardless.

So the method would have been public API surface, a second rendering path to
keep in step with the first, and a `#[allow(clippy::too_many_arguments)]` — in
exchange for a benefit no existing caller can take.

**Decision: rejected**, and recorded rather than deleted, because the reasoning
is worth keeping. ENGINEERING_PRINCIPLES #5 says performance work starts with a
measurement; this proposal was written from a measurement of the *copy* and
never checked whether removing it helped anyone. It did not.

**What would change this.** A caller that renders and consumes a frame on one
thread without needing ownership — a real-time preview painting straight to a
canvas, or an in-process encoder replacing the ffmpeg pipe. If either arrives,
reopen this with a benchmark showing the copy on the critical path.

The 0.394 ms copy is real, and still ~70% of the fixed per-frame cost. It is
simply not reachable through this API shape.

## Examples

Before, in the exporter:

```rust
let frame = self.renderer.render_frame(objects, states, w, h, bg, camera)?;
sink(&frame)?;                       // 8.3 MB allocated and freed per frame
```

After:

```rust
let mut buffer = vec![0u8; (w as usize) * (h as usize) * 4];   // once
for frame_idx in 0..total_frames {
    self.renderer.render_into(objects, states, w, h, bg, camera, &mut buffer)?;
    sink(&buffer)?;
}
```
