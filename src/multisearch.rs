use crate::CheckSum;
use crate::Checksums;
use std::collections::HashMap;


#[inline(always)]
pub fn weak_hash(v: u32) -> u16 {
    ((v >> 16) ^ ((v & 0xffff) * 62171)) as u16
}

#[derive(Debug, Default)]
pub struct Matcher {
    pub hash_table: HashMap<u16, HashMap<u32, Vec<usize>>>,
    pub strong_hashes: Vec<u128>,
    pub checksum: CheckSum
}


impl Matcher {

    pub fn new() -> Self {
        Self::default()
    }

    pub fn compile(&mut self, data: &[u8]) {
        let checksums = self.checksum.checksums(data).collect::<Vec<_>>();
        let mut hash_table = HashMap::new();

        for (offset, &checksum) in checksums.iter().enumerate() {

            let checksum_hash: u16 = weak_hash(checksum.0);

            hash_table
            .entry(checksum_hash)
            .and_modify(|m: &mut HashMap<u32, Vec<usize>>| {
                m
                .entry(checksum.0)
                .and_modify(|strong_hashes| {
                    strong_hashes.push(offset);
                })
                .or_insert(vec![offset]);
                
            })
            .or_insert_with(|| HashMap::from_iter([(checksum.0, vec![offset])]));
        }
        self.hash_table = hash_table;
        self.strong_hashes = checksums.iter().map(|&(_, strong)| strong).collect();
    }

    pub fn find_matches(&self, hashes_by_block: impl IntoIterator<Item=(u32, u128)>) -> Vec<(usize, usize)> {
        let mut matches = Vec::new();
        
        // For every incoming block (by hashes), find if there's a single block in the data provided
        // that matches the weak and strong checksums.

        for (byte_offset, (weak, strong)) in hashes_by_block.into_iter().enumerate() {
            let weak_16_bit_hash = weak_hash(weak);
            
            // First, check the 16-bit hash.
            if !self.hash_table.contains_key(&weak_16_bit_hash) {
                continue;
            }

            // Now, the 16-bit hash matches, so we need to check the 32-bit hash.
            let rolling_hash_map = self.hash_table.get(&weak_16_bit_hash).unwrap();
            if !rolling_hash_map.contains_key(&weak) {
                continue;
            }

            // Now, the 32-bit hash matches, so we need to check the 128-bit hash.
            let strong_hashes = rolling_hash_map.get(&weak).unwrap();

            for offset in strong_hashes {
                if self.strong_hashes[*offset] == strong {
                    matches.push((byte_offset, *offset));
                    break;
                }
            }
        }
        matches
    }

}


pub fn stuff() {
    let mut data = vec!["a"; 1_003].join("");
    data.push_str("b");

    let mut matcher = Matcher::new();
    matcher.compile(data.as_bytes());

    println!("{:#?}", matcher.hash_table);
    println!("{:#?}", matcher.strong_hashes);

    println!("matches: {:#?}", matcher.find_matches(
        vec![
            (3398728424, 184256362550517203141464663675311889247),
            (3398728424, 184256362550517203141464663675311889247),
            (3398728424, 184256362550517203141464663675311889247),
            (3398728424, 184256362550517203141464663675311889247),
            (3398793961, 121046634792758069105302070872080219867),
            (3398793962, 121046634792758069105302070872080219867),
        ]
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stuff() {
        stuff();
    }
}