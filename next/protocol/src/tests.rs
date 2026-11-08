use super::message::CHUNK_BYTES;
use super::*;

fn packet(message: Message) -> Vec<u8> {
    let mut output = [0; MAX_PACKET_BYTES];
    let length = message.encode(&mut output);
    output[..length].to_vec()
}

fn feed(receiver: &mut Receiver, bytes: &[u8]) -> Vec<Received> {
    bytes.iter().map(|byte| receiver.push(*byte)).filter(|received| *received != Received::Absorbed).collect()
}

fn encoded(input: &[u8]) -> Vec<u8> {
    let mut output = vec![0; input.len() + input.len() / COBS_MAX_RUN + 2];
    let length = cobs_encode(input, &mut output).unwrap();
    output[..length].to_vec()
}

#[test]
fn crc_matches_the_standard_check_value() {
    assert_eq!(crc16(b"123456789"), 0x29B1);
}

#[test]
fn cobs_matches_the_published_examples() {
    assert_eq!(encoded(&[0x00]), [0x01, 0x01]);
    assert_eq!(encoded(&[0x00, 0x00]), [0x01, 0x01, 0x01]);
    assert_eq!(encoded(&[0x00, 0x11, 0x00]), [0x01, 0x02, 0x11, 0x01]);
    assert_eq!(encoded(&[0x11, 0x22, 0x00, 0x33]), [0x03, 0x11, 0x22, 0x02, 0x33]);
    assert_eq!(encoded(&[0x11, 0x22, 0x33, 0x44]), [0x05, 0x11, 0x22, 0x33, 0x44]);
}

#[test]
fn cobs_handles_long_runs() {
    for length in [253, 254, 255, 256, 508, 509] {
        let input: Vec<u8> = (0..length).map(|index| (index % 255 + 1) as u8).collect();
        let wire = encoded(&input);
        assert!(!wire.contains(&0), "length {length}");
        let mut output = vec![0; length];
        assert_eq!(cobs_decode(&wire, &mut output), Some(length), "length {length}");
        assert_eq!(output, input, "length {length}");
    }
}

#[test]
fn cobs_round_trips_every_pattern_of_zeros() {
    for pattern in 0..256u32 {
        let input: Vec<u8> = (0..8).map(|bit| if pattern >> bit & 1 == 0 { 0 } else { 0x55 }).collect();
        let wire = encoded(&input);
        assert!(!wire.contains(&0), "{input:?}");
        let mut output = [0xEE; 8];
        assert_eq!(cobs_decode(&wire, &mut output), Some(8), "{input:?}");
        assert_eq!(output.as_slice(), input, "{input:?}");
    }
}

#[test]
fn cobs_refuses_a_zero_or_a_run_past_the_end() {
    let mut output = [0; 8];
    assert_eq!(cobs_decode(&[0x03, 0x11, 0x00], &mut output), None);
    assert_eq!(cobs_decode(&[0x05, 0x11], &mut output), None);
}

#[test]
fn a_packet_has_this_exact_form_on_the_wire() {
    // Pinned, as `tools/check_images.py` encodes packets separately in Python.
    assert_eq!(packet(Message::MouseMove { dx: 300, dy: -2 }), [0x00, 0x08, 0x01, 0x2C, 0x01, 0xFE, 0xFF, 0x2F, 0xDA, 0x00]);
}

#[test]
fn a_message_comes_back_out() {
    for (dx, dy) in [(0, 0), (1, -1), (-300, 300), (i16::MAX, i16::MIN), (256, 512)] {
        let message = Message::MouseMove { dx, dy };
        assert_eq!(feed(&mut Receiver::new(), &packet(message)), [Received::Message(message)], "{dx} {dy}");
    }
}

#[test]
fn a_key_comes_back_out() {
    for key in [0, b'0', b'4', 0xFF] {
        let message = Message::Key(key);
        assert_eq!(feed(&mut Receiver::new(), &packet(message)), [Received::Message(message)], "{key}");
    }
}

#[test]
fn a_message_of_the_wrong_length_is_dropped() {
    for plain in [&[2u8][..], &[2, 1, 2], &[1, 1, 2, 3], &[1, 1, 2, 3, 4, 5], &[3, 0, 1], &[3, 9, 0, 0, 0, 0, 0, 0, 0, 0], &[4, 0, 0, 0, 0], &[5, 0], &[6, 0, 0, 0, 0, 9], &[7], &[7, 2], &[7, 0, 0], &[8, 9, 0, 0, 0, 0, 0, 0, 0, 0], &[8, 0]] {
        let mut message = plain.to_vec();
        message.extend(crc16(plain).to_le_bytes());
        let mut bytes = vec![BOUNDARY];
        bytes.extend(encoded(&message));
        bytes.push(BOUNDARY);
        assert_eq!(feed(&mut Receiver::new(), &bytes), [], "{plain:?}");
    }
}

#[test]
fn characters_pass_through_around_packets() {
    let message = Message::MouseMove { dx: 5, dy: 6 };
    let mut bytes = b"ab".to_vec();
    bytes.extend(packet(message));
    bytes.push(b'c');
    let got = feed(&mut Receiver::new(), &bytes);
    assert_eq!(got, [Received::Character(b'a'), Received::Character(b'b'), Received::Message(message), Received::Character(b'c')]);
}

#[test]
fn a_corrupted_packet_is_dropped_and_the_next_one_still_arrives() {
    let message = Message::MouseMove { dx: 5, dy: 6 };
    let good = packet(message);
    for index in 1..good.len() - 1 {
        for flip in [0x01, 0x80, 0xFF] {
            let mut bad = good.clone();
            bad[index] ^= flip;
            let mut bytes = bad.clone();
            bytes.extend(&good);
            let got = feed(&mut Receiver::new(), &bytes);
            assert_eq!(got.last(), Some(&Received::Message(message)), "byte {index} flip {flip:#x}");
            assert!(got.iter().filter(|received| matches!(received, Received::Message(_))).all(|received| *received == Received::Message(message)), "byte {index} flip {flip:#x} gave a wrong message: {got:?}");
        }
    }
}

#[test]
fn losing_a_boundary_recovers_by_the_following_packet() {
    // The next packet's contents come out as characters, as nothing marks them as a
    // packet's. That's why commands are packets, and characters are only text.
    let message = Message::MouseMove { dx: 5, dy: 6 };
    let good = packet(message);
    let mut bytes = good[..good.len() - 1].to_vec();
    bytes.extend(&good);
    bytes.extend(&good);
    let got = feed(&mut Receiver::new(), &bytes);
    assert_eq!(got.first(), Some(&Received::Message(message)));
    assert_eq!(got.last(), Some(&Received::Message(message)));
    assert!(got.iter().any(|received| matches!(received, Received::Character(_))));
}

#[test]
fn an_endless_packet_can_not_overflow() {
    let mut receiver = Receiver::new();
    receiver.push(BOUNDARY);
    for _ in 0..MAX_PACKET_BYTES * 3 {
        receiver.push(0x55);
    }
    let message = Message::MouseMove { dx: 1, dy: 2 };
    assert_eq!(feed(&mut receiver, &packet(message)).last(), Some(&Received::Message(message)));
}

#[test]
fn abandoning_a_stray_start_lets_characters_through_again() {
    let mut receiver = Receiver::new();
    receiver.push(BOUNDARY);
    assert!(receiver.in_packet());
    receiver.abandon();
    assert_eq!(receiver.push(b'x'), Received::Character(b'x'));
}

#[test]
fn an_unknown_type_with_a_good_crc_is_dropped() {
    let plain = [0x7F, 0x01, 0x02];
    let mut message = plain.to_vec();
    message.extend(crc16(&plain).to_le_bytes());
    let mut bytes = vec![BOUNDARY];
    bytes.extend(encoded(&message));
    bytes.push(BOUNDARY);
    assert_eq!(feed(&mut Receiver::new(), &bytes), []);
}

#[test]
fn crc32_matches_the_standard_check_value() {
    assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
}

#[test]
fn upload_messages_come_back_out() {
    let mut data = [0; CHUNK_BYTES];
    for (index, byte) in data.iter_mut().enumerate() {
        // Every value, including the zeros COBS has to remove.
        *byte = index as u8;
    }
    let mut short = [0; CHUNK_BYTES];
    short[0] = 7;
    let messages = [
        Message::UploadStart { target: Target::Scratch, size: 524_288, crc32: 0xDEAD_BEEF },
        Message::UploadStart { target: Target::Kernel, size: 0, crc32: 0 },
        Message::UploadStart { target: Target::Trial, size: 1, crc32: 2 },
        Message::Reboot { trial: false },
        Message::InstallFromMemory { target: Target::Kernel, size: 81_120, crc32: 0x1234_5678 },
        Message::Reboot { trial: true },
        Message::UploadChunk { offset: 123_456, length: CHUNK_BYTES as u8, data },
        Message::UploadChunk { offset: 0, length: 1, data: short },
        Message::UploadCommit,
        Message::UploadReply { next_offset: 240, status: UploadStatus::Receiving },
        Message::UploadReply { next_offset: 0, status: UploadStatus::Done },
        Message::UploadReply { next_offset: 7, status: UploadStatus::Refused },
        Message::UploadReply { next_offset: 7, status: UploadStatus::BadChecksum },
        Message::UploadReply { next_offset: 7, status: UploadStatus::WriteFailed },
    ];
    for message in messages {
        assert_eq!(feed(&mut Receiver::new(), &packet(message)), [Received::Message(message)], "{message:?}");
    }
}

#[test]
fn an_empty_chunk_is_dropped() {
    let plain = [4, 0, 0, 0, 0];
    let mut message = plain.to_vec();
    message.extend(crc16(&plain).to_le_bytes());
    assert_eq!(decode_packet(&encoded(&message)), None);
}

#[test]
fn a_chunks_length_only_covers_its_own_data() {
    let mut data = [0x55; CHUNK_BYTES];
    data[3] = 0x66;
    let sent = Message::UploadChunk { offset: 1, length: 4, data };
    let [Received::Message(Message::UploadChunk { length, data: got, .. })] = feed(&mut Receiver::new(), &packet(sent))[..] else {
        panic!("no chunk came back");
    };
    assert_eq!(length, 4);
    assert_eq!(got[..4], data[..4]);
    assert_eq!(got[4], 0);
}
