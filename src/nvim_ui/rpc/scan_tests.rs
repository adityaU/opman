use super::*;

fn samples() -> Vec<Vec<u8>> {
    let mut result = vec![
        vec![0x7f],
        vec![0xe0],
        vec![0xcc, 0xff],
        vec![0xcd, 0x12, 0x34],
        vec![0xce, 1, 2, 3, 4],
        vec![0xcf, 1, 2, 3, 4, 5, 6, 7, 8],
    ];
    result.extend([
        vec![0xd0, 0x80],
        vec![0xd1, 0x80, 0x01],
        vec![0xd2, 0x80, 0, 0, 1],
        vec![0xd3, 0x80, 0, 0, 0, 0, 0, 0, 1],
        vec![0xca, 0, 0, 0, 0],
        vec![0xcb, 0, 0, 0, 0, 0, 0, 0, 0],
    ]);
    result.extend([
        vec![0xa0],
        vec![0xa3, b'f', b'o', b'o'],
        {
            let mut v = vec![0xd9, 32];
            v.extend([b'x'; 32]);
            v
        },
        {
            let mut v = vec![0xda, 0, 1];
            v.push(b'x');
            v
        },
        {
            let v = vec![0xdb, 0, 0, 0, 1, b'x'];
            v
        },
    ]);
    result.extend([
        vec![0xc4, 2, 1, 2],
        vec![0xc5, 0, 2, 1, 2],
        vec![0xc6, 0, 0, 0, 2, 1, 2],
    ]);
    result.extend([
        vec![0x90],
        vec![0x92, 1, 2],
        vec![0xdc, 0, 1, 1],
        vec![0xdd, 0, 0, 0, 1, 1],
    ]);
    result.extend([
        vec![0x80],
        vec![0x81, 1, 2],
        vec![0xde, 0, 1, 1, 2],
        vec![0xdf, 0, 0, 0, 1, 1, 2],
    ]);
    result.extend([
        vec![0xd4, 1, 9],
        vec![0xd5, 1, 9, 8],
        vec![0xd6, 1, 9, 8, 7, 6],
        vec![0xd7, 1, 1, 2, 3, 4, 5, 6, 7, 8],
        {
            let mut v = vec![0xd8, 1];
            v.extend([0; 16]);
            v
        },
        vec![0xc7, 1, 1, 9],
        vec![0xc8, 0, 1, 1, 9],
        vec![0xc9, 0, 0, 0, 1, 1, 9],
    ]);
    result.extend([
        vec![0xc0],
        vec![0xc2],
        vec![0xc3],
        vec![0x91, 0x81, 0xa1, b'a', 0x93, 1, 2, 3],
    ]);
    result
}

#[test]
fn every_marker_family_is_consumed() {
    for value in samples() {
        let mut cursor = 0;
        assert_eq!(skip_value(&value, &mut cursor), Ok(()), "{value:02x?}");
        assert_eq!(cursor, value.len());
    }
}

#[test]
fn all_truncations_are_incomplete() {
    for value in samples() {
        for end in 0..value.len() {
            let mut cursor = 0;
            assert_eq!(
                skip_value(&value[..end], &mut cursor),
                Err(ScanError::Incomplete),
                "end={end} value={value:02x?}"
            );
            assert_eq!(cursor, 0);
        }
    }
}

#[test]
fn differential_oracle_matches_rmpv() {
    for i in 0..1024u64 {
        let value = match i % 8 {
            0 => rmpv::Value::from(i as i64),
            1 => rmpv::Value::from(-(i as i64) - 1),
            2 => rmpv::Value::Array(vec![
                rmpv::Value::from(i),
                rmpv::Value::Nil,
                rmpv::Value::Boolean(i % 2 == 0),
            ]),
            3 => rmpv::Value::Map(vec![(rmpv::Value::from("key"), rmpv::Value::from(i))]),
            4 => rmpv::Value::F32(i as f32),
            5 => rmpv::Value::F64(i as f64 / 3.0),
            6 => rmpv::Value::Binary(vec![i as u8, (i >> 8) as u8]),
            _ => rmpv::Value::from(format!("value-{i}")),
        };
        let mut encoded = Vec::new();
        rmpv::encode::write_value(&mut encoded, &value).unwrap();
        let mut cursor = 0;
        skip_value(&encoded, &mut cursor).unwrap();
        let mut input = encoded.as_slice();
        let _ = rmpv::decode::read_value_ref(&mut input).unwrap();
        assert_eq!(cursor, encoded.len() - input.len());
    }
}

#[test]
fn scalar_readers_are_transactional() {
    let mut cursor = 0;
    assert_eq!(read_array_len(&[0x93], &mut cursor), Ok(3));
    cursor = 0;
    assert_eq!(
        read_uint(&[0xcf, 0, 0, 0, 0, 0, 0, 0, 9], &mut cursor),
        Ok(9)
    );
    let mut text_cursor = 0;
    assert_eq!(
        read_str_slice(&[0xa3, b'a', b'b', b'c'], &mut text_cursor),
        Ok("abc")
    );
    let mut bad = 0;
    assert_eq!(
        read_str_slice(&[0xd9, 2, 0xff], &mut bad),
        Err(ScanError::Incomplete)
    );
    assert_eq!(bad, 0);
}

#[test]
fn reserved_marker_is_malformed() {
    let mut cursor = 0;
    assert_eq!(skip_value(&[0xc1], &mut cursor), Err(ScanError::Malformed));
}
