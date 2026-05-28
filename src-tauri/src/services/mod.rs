//! Business logic services. Each module is pure (functional) where
//! possible — input state, output new state, no shared mutation.
//!
//!   - `operations` — caption split/merge/edit/shift (Phase 3.1)
//!   - `export`     — SRT/VTT/ASS writers (Phase 6.1)

pub mod operations;
pub mod export;
