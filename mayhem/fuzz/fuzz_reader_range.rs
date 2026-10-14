#![no_main]

use std::ops::Bound;

use libfuzzer_sys::Corpus;
use libfuzzer_sys::fuzz_target;
use opendal::Operator;
use opendal::services::Memory;
use opendal::tests::TEST_RUNTIME;

const DATA_LEN: usize = 16;

fn bound(kind: u8, raw: u64) -> Bound<u64> {
    let value = match (kind >> 2) & 3 {
        0 | 1 => raw % 32,
        2 => u64::MAX - raw % 4,
        _ => raw,
    };
    match kind & 3 {
        0 | 3 => Bound::Included(value),
        1 => Bound::Excluded(value),
        _ => Bound::Unbounded,
    }
}

// Input: chunk(1) op(1) start_kind(1) start(8) end_kind(1) end(8)
fuzz_target!(|data: &[u8]| -> Corpus {
    if data.len() < 20 {
        return Corpus::Reject;
    }
    let chunk = 1 + data[0] as usize;
    let use_read_into = data[1] & 1 == 1;
    let start = bound(data[2], u64::from_le_bytes(data[3..11].try_into().unwrap()));
    let end = bound(data[11], u64::from_le_bytes(data[12..20].try_into().unwrap()));

    let content: Vec<u8> = (0..DATA_LEN as u8).collect();

    TEST_RUNTIME.block_on(async {
        let op = Operator::new(Memory::default()).unwrap().finish();
        op.write("test", content.clone()).await.unwrap();
        let reader = op.reader_with("test").chunk(chunk).await.unwrap();

        let result = if use_read_into {
            let mut buf = Vec::new();
            reader
                .read_into(&mut buf, (start, end))
                .await
                .map(|_| buf)
        } else {
            reader.read((start, end)).await.map(|b| b.to_vec())
        };

        // Where the range lies inside the object, the bytes read are exactly its bytes.
        let s: u128 = match start {
            Bound::Included(v) => v as u128,
            Bound::Excluded(v) => v as u128 + 1,
            Bound::Unbounded => 0,
        };
        let e: u128 = match end {
            Bound::Included(v) => v as u128 + 1,
            Bound::Excluded(v) => v as u128,
            Bound::Unbounded => DATA_LEN as u128,
        };
        if let Ok(bytes) = result {
            if s <= e && e <= DATA_LEN as u128 {
                assert_eq!(bytes, content[s as usize..e as usize]);
            }
        }
    });
    Corpus::Keep
});
