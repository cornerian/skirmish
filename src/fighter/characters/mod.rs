//! Pure per-character scalar arithmetic. Each character gets one module
//! here holding only its own numeric special-move helpers (entry blends,
//! turn checks, input gates); resource shapes, dispatch and state transitions
//! live in `game::characters`, which calls into these functions.

pub mod fox;
