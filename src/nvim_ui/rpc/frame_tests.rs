use super::*;

fn messages() -> Vec<Vec<u8>> {
    vec![
        vec![
            0x94, 0, 7, 0xa6, b'r', b'e', b'd', b'r', b'a', b'w', 0x91, 1,
        ],
        vec![0x93, 2, 0xa4, b't', b'e', b's', b't', 0x90],
        vec![0x94, 1, 7, 0xc0, 0xa2, b'o', b'k'],
    ]
}

fn collect(framer: &mut Framer) -> Vec<(MessageKind, Option<u64>, Vec<u8>, Vec<u8>)> {
    let mut result = Vec::new();
    while let Some(span) = framer.next().unwrap() {
        result.push((
            span.kind,
            span.msgid,
            framer.data()[span.method_range.clone()].to_vec(),
            framer.data()[span.params_range.clone()].to_vec(),
        ));
    }
    result
}

#[test]
fn one_byte_at_a_time_matches_whole_buffer() {
    let all = messages().into_iter().flatten().collect::<Vec<_>>();
    let mut whole = Framer::new();
    whole.push(&all);
    let expected = collect(&mut whole);

    let mut incremental = Framer::new();
    let mut actual = Vec::new();
    for byte in all {
        incremental.push(&[byte]);
        actual.extend(collect(&mut incremental));
    }
    assert_eq!(actual, expected);
}

#[test]
fn spans_identify_methods_and_params_without_copying_in_the_framer() {
    let mut framer = Framer::new();
    framer.push(&messages()[0]);
    let span = framer.next().unwrap().unwrap();
    assert_eq!(span.kind, MessageKind::Request);
    assert_eq!(span.msgid, Some(7));
    assert_eq!(&framer.data()[span.method_range], b"redraw");
    assert_eq!(&framer.data()[span.params_range], &[0x91, 1]);
    assert_eq!(framer.pending_len(), 0);
}

#[test]
fn garbage_is_malformed_and_partial_is_retained() {
    let mut framer = Framer::new();
    framer.push(&[0xc1]);
    assert_eq!(framer.next(), Err(FrameError::Malformed));

    let mut partial = Framer::new();
    partial.push(&[0x94, 0, 1]);
    assert_eq!(partial.next(), Ok(None));
    assert_eq!(partial.pending_len(), 3);
}

#[test]
fn oversized_message_is_rejected() {
    let mut framer = Framer::with_max_message_len(4);
    framer.push(&messages()[0]);
    assert_eq!(
        framer.next(),
        Err(FrameError::Oversized { length: 12, max: 4 })
    );
}

#[test]
fn oversized_length_header_is_rejected_before_payload_arrives() {
    let mut framer = Framer::with_max_message_len(16);
    framer.push(&[0xdd, 0xff, 0xff, 0xff, 0xff]);
    assert_eq!(
        framer.next(),
        Err(FrameError::Oversized {
            length: u32::MAX as usize,
            max: 16
        })
    );
}

#[test]
fn array_length_prefix_can_be_split_across_reads() {
    let message = messages()[0].clone();
    let mut framer = Framer::new();
    for (index, byte) in message.iter().copied().enumerate() {
        framer.push(&[byte]);
        if index + 1 < message.len() {
            assert_eq!(framer.next(), Ok(None));
        }
    }
    assert!(framer.next().unwrap().is_some());
}
