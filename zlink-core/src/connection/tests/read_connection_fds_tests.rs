//! Unit tests for ReadConnection file descriptor passing.

#![cfg(test)]

use crate::{
    connection::{read_connection::ReadConnection, socket::Socket},
    test_utils::mock_socket::MockSocket,
};
use alloc::vec::Vec;
use rustix::fd::AsFd;
use serde::{Deserialize, Serialize};
use std::os::unix::net::UnixStream;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "method", content = "parameters")]
enum TestMethod {
    #[serde(rename = "org.example.Test")]
    Test { value: u32 },
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct TestReply {
    result: String,
}

#[tokio::test]
async fn receive_call_with_single_fd() {
    let (r_fd, _w_fd) = UnixStream::pair().unwrap();

    let call_json = r#"{"method":"org.example.Test","parameters":{"value":42}}"#;
    let fds = vec![vec![r_fd.into()]];

    let socket = MockSocket::new(&[call_json], fds);
    let (read, _write) = socket.split();
    let mut conn = ReadConnection::new(read, 0);

    let (call, received_fds) = conn.receive_call::<TestMethod>().await.unwrap();

    assert_eq!(call.method(), &TestMethod::Test { value: 42 });
    assert_eq!(received_fds.len(), 1);
}

#[tokio::test]
async fn receive_call_with_multiple_fds() {
    let (r1, _w1) = UnixStream::pair().unwrap();
    let (r2, _w2) = UnixStream::pair().unwrap();
    let (r3, _w3) = UnixStream::pair().unwrap();

    let call_json = r#"{"method":"org.example.Test","parameters":{"value":42}}"#;
    let fds = vec![vec![r1.into(), r2.into(), r3.into()]];

    let socket = MockSocket::new(&[call_json], fds);
    let (read, _write) = socket.split();
    let mut conn = ReadConnection::new(read, 0);

    let (call, received_fds) = conn.receive_call::<TestMethod>().await.unwrap();

    assert_eq!(call.method(), &TestMethod::Test { value: 42 });
    assert_eq!(received_fds.len(), 3);
}

#[tokio::test]
async fn receive_call_with_no_fds() {
    let call_json = r#"{"method":"org.example.Test","parameters":{"value":42}}"#;
    let fds = vec![vec![]];

    let socket = MockSocket::new(&[call_json], fds);
    let (read, _write) = socket.split();
    let mut conn = ReadConnection::new(read, 0);

    let (call, received_fds) = conn.receive_call::<TestMethod>().await.unwrap();

    assert_eq!(call.method(), &TestMethod::Test { value: 42 });
    assert!(received_fds.is_empty());
}

#[tokio::test]
async fn receive_multiple_calls_with_mixed_fds() {
    let (r1, _w1) = UnixStream::pair().unwrap();
    let (r2, _w2) = UnixStream::pair().unwrap();

    let call1 = r#"{"method":"org.example.Test","parameters":{"value":1}}"#;
    let call2 = r#"{"method":"org.example.Test","parameters":{"value":2}}"#;
    let call3 = r#"{"method":"org.example.Test","parameters":{"value":3}}"#;

    let fds = vec![vec![r1.into(), r2.into()]];

    let socket = MockSocket::new(&[call1, call2, call3], fds);
    let (read, _write) = socket.split();
    let mut conn = ReadConnection::new(read, 0);

    let (call, fds) = conn.receive_call::<TestMethod>().await.unwrap();
    assert_eq!(call.method(), &TestMethod::Test { value: 1 });
    assert_eq!(fds.len(), 2); // All FDs from the batch

    let (call, fds) = conn.receive_call::<TestMethod>().await.unwrap();
    assert_eq!(call.method(), &TestMethod::Test { value: 2 });
    assert!(fds.is_empty()); // No new read, so no new FDs

    let (call, fds) = conn.receive_call::<TestMethod>().await.unwrap();
    assert_eq!(call.method(), &TestMethod::Test { value: 3 });
    assert!(fds.is_empty()); // No new read, so no new FDs
}

#[tokio::test]
async fn receive_reply_with_single_fd() {
    let (r_fd, _w_fd) = UnixStream::pair().unwrap();

    let reply_json = r#"{"parameters":{"result":"success"}}"#;
    let fds = vec![vec![r_fd.into()]];

    let socket = MockSocket::new(&[reply_json], fds);
    let (read, _write) = socket.split();
    let mut conn = ReadConnection::new(read, 0);

    let (reply, received_fds) = conn.receive_reply::<TestReply, TestReply>().await.unwrap();

    let reply = reply.unwrap();
    assert_eq!(reply.parameters().unwrap().result, "success");

    assert_eq!(received_fds.len(), 1);
}

#[tokio::test]
async fn receive_reply_with_multiple_fds() {
    let (r1, _w1) = UnixStream::pair().unwrap();
    let (r2, _w2) = UnixStream::pair().unwrap();

    let reply_json = r#"{"parameters":{"result":"success"}}"#;
    let fds = vec![vec![r1.into(), r2.into()]];

    let socket = MockSocket::new(&[reply_json], fds);
    let (read, _write) = socket.split();
    let mut conn = ReadConnection::new(read, 0);

    let (reply, received_fds) = conn.receive_reply::<TestReply, TestReply>().await.unwrap();

    let reply = reply.unwrap();
    assert_eq!(reply.parameters().unwrap().result, "success");
    assert_eq!(received_fds.len(), 2);
}

#[tokio::test]
async fn receive_reply_with_no_fds() {
    let reply_json = r#"{"parameters":{"result":"success"}}"#;
    let fds = vec![vec![]];

    let socket = MockSocket::new(&[reply_json], fds);
    let (read, _write) = socket.split();
    let mut conn = ReadConnection::new(read, 0);

    let (reply, received_fds) = conn.receive_reply::<TestReply, TestReply>().await.unwrap();

    let reply = reply.unwrap();
    assert_eq!(reply.parameters().unwrap().result, "success");
    assert!(received_fds.is_empty());
}

#[tokio::test]
async fn receive_call_malformed_json() {
    let (r_fd, _w_fd) = UnixStream::pair().unwrap();

    let malformed = r#"{"method":"org.example.Test","parameters":"#;
    let fds = vec![vec![r_fd.into()]];

    let socket = MockSocket::new(&[malformed], fds);
    let (read, _write) = socket.split();
    let mut conn = ReadConnection::new(read, 0);

    let result = conn.receive_call::<TestMethod>().await;
    assert!(matches!(result, Err(crate::Error::Json(_))));
}

#[tokio::test]
async fn receive_call_large_message() {
    let (r_fd, _w_fd) = UnixStream::pair().unwrap();

    let large_value = 1000000u32;
    let call_json = format!(
        r#"{{"method":"org.example.Test","parameters":{{"value":{}}}}}"#,
        large_value
    );

    let fds = vec![vec![r_fd.into()]];

    let socket = MockSocket::new(&[&call_json], fds);
    let (read, _write) = socket.split();
    let mut conn = ReadConnection::new(read, 0);

    let (call, received_fds) = conn.receive_call::<TestMethod>().await.unwrap();

    assert_eq!(call.method(), &TestMethod::Test { value: large_value });
    assert_eq!(received_fds.len(), 1);
}

#[tokio::test]
async fn fd_validity_after_receive() {
    use rustix::io::{read, write};

    let (r_fd, w_fd) = UnixStream::pair().unwrap();

    write(w_fd.as_fd(), b"test data").unwrap();

    let call_json = r#"{"method":"org.example.Test","parameters":{"value":42}}"#;
    let fds = vec![vec![r_fd.into()]];

    let socket = MockSocket::new(&[call_json], fds);
    let (read_half, _write) = socket.split();
    let mut conn = ReadConnection::new(read_half, 0);

    let (_call, received_fds) = conn.receive_call::<TestMethod>().await.unwrap();

    let mut buf = [0u8; 9];
    let n = read(received_fds[0].as_fd(), &mut buf).unwrap();
    assert_eq!(n, 9);
    assert_eq!(&buf[..n], b"test data");
}

#[tokio::test]
async fn receive_call_with_max_fds() {
    let mut fds_vec = Vec::new();
    for _ in 0..10 {
        let (r, _w) = UnixStream::pair().unwrap();
        fds_vec.push(r.into());
    }

    let call_json = r#"{"method":"org.example.Test","parameters":{"value":42}}"#;
    let fds = vec![fds_vec];

    let socket = MockSocket::new(&[call_json], fds);
    let (read, _write) = socket.split();
    let mut conn = ReadConnection::new(read, 0);

    let (_call, received_fds) = conn.receive_call::<TestMethod>().await.unwrap();
    assert_eq!(received_fds.len(), 10);
}

#[tokio::test]
async fn receive_call_backward_compat_regular_receive() {
    let (r_fd, _w_fd) = UnixStream::pair().unwrap();

    let call_json = r#"{"method":"org.example.Test","parameters":{"value":42}}"#;
    let fds = vec![vec![r_fd.into()]];

    let socket = MockSocket::new(&[call_json], fds);
    let (read, _write) = socket.split();
    let mut conn = ReadConnection::new(read, 0);

    let (call, _fds) = conn.receive_call::<TestMethod>().await.unwrap();

    assert_eq!(call.method(), &TestMethod::Test { value: 42 });
}

#[tokio::test]
async fn mixed_receive_call_and_receive_call() {
    let (r1, _w1) = UnixStream::pair().unwrap();

    let call1 = r#"{"method":"org.example.Test","parameters":{"value":1}}"#;
    let call2 = r#"{"method":"org.example.Test","parameters":{"value":2}}"#;

    let fds = vec![vec![r1.into()], vec![]];

    let socket = MockSocket::new(&[call1, call2], fds);
    let (read, _write) = socket.split();
    let mut conn = ReadConnection::new(read, 0);

    let (call, fds) = conn.receive_call::<TestMethod>().await.unwrap();
    assert_eq!(call.method(), &TestMethod::Test { value: 1 });
    assert_eq!(fds.len(), 1);

    let (call, _fds) = conn.receive_call::<TestMethod>().await.unwrap();
    assert_eq!(call.method(), &TestMethod::Test { value: 2 });
}

#[tokio::test]
async fn receive_reply_error_with_fds() {
    use crate::ReplyError;

    #[derive(Debug, ReplyError)]
    #[zlink(interface = "org.example")]
    enum TestError {
        NotFound { code: i32 },
    }

    let (r_fd, _w_fd) = UnixStream::pair().unwrap();

    let error_json = r#"{"error":"org.example.NotFound","parameters":{"code":404}}"#;
    let fds = vec![vec![r_fd.into()]];

    let socket = MockSocket::new(&[error_json], fds);
    let (read, _write) = socket.split();
    let mut conn = ReadConnection::new(read, 0);

    let (reply, received_fds) = conn.receive_reply::<TestReply, TestError>().await.unwrap();

    assert!(reply.is_err());
    assert!(matches!(
        reply.unwrap_err(),
        TestError::NotFound { code: 404 }
    ));
    assert_eq!(received_fds.len(), 1);
}
