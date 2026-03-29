pub fn current() -> &'static str {
    option_env!("ALCHEMIST_BUILD_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_version_is_non_empty() {
        assert!(!current().trim().is_empty());
    }
}
