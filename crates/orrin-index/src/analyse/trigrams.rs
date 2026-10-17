const BITSET_FROM: usize = 64 * 1024;

pub fn of_content(bytes: &[u8]) -> Vec<u32> {
    let grams = bytes.windows(3).filter(|w| !w.contains(&b'\n')).map(pack);
    if bytes.len() >= BITSET_FROM {
        return distinct(grams);
    }
    let mut grams: Vec<u32> = grams.collect();
    grams.sort_unstable();
    grams.dedup();
    grams
}

fn distinct(grams: impl Iterator<Item = u32>) -> Vec<u32> {
    let mut seen = vec![0u64; 1 << 18];
    for gram in grams {
        seen[(gram >> 6) as usize] |= 1 << (gram & 63);
    }
    let mut out = Vec::new();
    for (at, &word) in seen.iter().enumerate() {
        let mut rest = word;
        while rest != 0 {
            out.push(((at as u32) << 6) | rest.trailing_zeros());
            rest &= rest - 1;
        }
    }
    out
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

    #[test]
    fn big_content_gives_the_same_grams() {
        let text: String = (0..20_000).map(|i| format!("line {i} Value_{}\n", i % 97)).collect();
        let mut sorted: Vec<u32> = text.as_bytes().windows(3).filter(|w| !w.contains(&b'\n')).map(pack).collect();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(of_content(text.as_bytes()), sorted);
    }
}
