//! Mimir core crate.
//!
//! Phase 0 (CI Bootstrap) ships only a placeholder so the workspace builds.
//! Real implementation lands in Tier 0.

pub fn hello() -> &'static str {
    "mimir-core"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_returns_crate_name() {
        assert_eq!(hello(), "mimir-core");
    }
}
