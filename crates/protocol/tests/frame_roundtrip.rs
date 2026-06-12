use mobilecode_connect_protocol::{DataStreamHeader, ServiceId, SessionId, StreamId};

#[test]
fn data_stream_header_roundtrips_with_length_prefix() {
    let header = DataStreamHeader {
        stream_id: StreamId::new("stream_001"),
        session_id: SessionId::new("sess_001"),
        service_id: ServiceId::new("svc_web_3000"),
    };

    let bytes = header.encode_with_len_prefix().unwrap();
    let decoded = DataStreamHeader::decode_with_len_prefix(&bytes).unwrap();

    assert_eq!(decoded, header);
}

#[test]
fn data_stream_header_rejects_short_length_prefix() {
    let err = DataStreamHeader::decode_with_len_prefix(&[0, 0, 0]).unwrap_err();

    assert!(err.to_string().contains("length prefix"));
}

#[test]
fn data_stream_header_rejects_oversized_header() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(65_537_u32).to_be_bytes());
    bytes.resize(65_541, b' ');

    let err = DataStreamHeader::decode_with_len_prefix(&bytes).unwrap_err();

    assert!(err.to_string().contains("too large"));
}
