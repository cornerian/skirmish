//! Pure per-character scalar arithmetic. Each character gets one module
//! here holding only its own numeric special-move helpers (entry blends,
//! turn checks, input gates); resource shapes, dispatch and state
//! transitions live in the character's own subtree (`characters::fox`),
//! which calls into these functions.

pub mod fox;
