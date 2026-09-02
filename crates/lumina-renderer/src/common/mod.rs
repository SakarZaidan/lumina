//! Backend-neutral helpers shared by the Skia (CPU) and Vello (GPU)
//! renderers (TD-02).
//!
//! Both backends must produce the same pixels for the same scene (verified
//! by the `backend_parity` test suite), so everything that *decides* what
//! to draw — parsing, geometry construction, ordering, transform math —
//! lives here exactly once. The backends keep only the code that *emits*
//! draw calls to their respective libraries.

pub(crate) mod color;
pub(crate) mod fill;
pub(crate) mod path;
pub(crate) mod plot;
pub(crate) mod scene;
pub(crate) mod shadow;
pub(crate) mod stroke;
pub(crate) mod ticks;
