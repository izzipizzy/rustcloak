use rand::Rng;

/// A fresh random fingerprint seed.
pub fn gen_seed() -> u64 {
    rand::thread_rng().gen()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_seeds_differ() {
        // Probability of collision across u64 is negligible.
        assert_ne!(gen_seed(), gen_seed());
    }
}
