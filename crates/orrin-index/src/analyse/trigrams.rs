pub fn of_content(bytes: &[u8]) -> Vec<u32> {
    let mut grams: Vec<u32> = bytes.windows(3).filter(|w| !w.contains(&b'\n')).map(pack).collect();
    grams.sort_unstable();
    grams.dedup();
    grams
}

pub fn of_literal(bytes: &[u8], case_sensitive: bool) -> Vec<u32> {
    let usable = |w: &&[u8]| !w.contains(&b'\n') && (case_sensitive || w.iter().all(u8::is_ascii));
    let mut grams: Vec<u32> = bytes.windows(3).filter(usable).map(pack).collect();
    grams.sort_unstable();
    grams.dedup();
    grams
}

pub fn encode(grams: &[u32]) -> Vec<u8> {
    grams.iter().flat_map(|g| g.to_le_bytes()[..3].to_vec()).collect()
}

pub fn decode(bytes: &[u8]) -> Vec<u32> {
    bytes.chunks_exact(3).map(|c| u32::from_le_bytes([c[0], c[1], c[2], 0])).collect()
}

fn pack(w: &[u8]) -> u32 {
    let [a, b, c] = [w[0], w[1], w[2]].map(|b| b.to_ascii_lowercase() as u32);
    a | (b << 8) | (c << 16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn literal_grams_are_found_in_content_regardless_of_case() {
        let content = of_content(b"fn Hello_World() {}\nnext");
        let query = of_literal(b"hello_world", false);
        assert!(!query.is_empty());
        assert!(query.iter().all(|g| content.binary_search(g).is_ok()));
        assert_eq!(decode(&encode(&content)), content);
    }
}
