use std::hash::BuildHasher;

use orrin_core::Hash;
use orrin_core::id::fresh;

pub fn secret() -> String {
    let seed = fresh("secret");
    let mut bytes = seed.clone().into_bytes();
    for round in 0..4u8 {
        let random = std::collections::hash_map::RandomState::new().hash_one((&seed, round));
        bytes.extend_from_slice(&random.to_le_bytes());
    }
    Hash::of(&bytes).as_str()[..40].to_string()
}

pub fn same(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_differ_and_compare() {
        let a = secret();
        assert_ne!(a, secret());
        assert!(same(&a, &a.clone()));
        assert!(!same(&a, "nope"));
    }
}
