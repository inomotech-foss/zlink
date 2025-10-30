//! Helper functions for Unix socket FD passing.
//!
//! These are public but hidden from documentation as they're implementation details shared between
//! runtime-specific socket implementations.

use core::mem::MaybeUninit;
use std::{
    io,
    os::fd::{AsFd, BorrowedFd, OwnedFd},
};

/// Receive a message from a Unix socket, including any file descriptors.
///
/// This is a low-level helper that performs the `recvmsg` syscall.
#[doc(hidden)]
pub fn recvmsg(fd: impl AsFd, buf: &mut [u8]) -> io::Result<(usize, alloc::vec::Vec<OwnedFd>)> {
    use rustix::net::{recvmsg, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags};
    use std::io::IoSliceMut;

    let mut cmsg_buf = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(MAX_FDS))];
    let mut control = RecvAncillaryBuffer::new(&mut cmsg_buf);

    let mut iov = [IoSliceMut::new(buf)];
    recvmsg(fd.as_fd(), &mut iov, &mut control, RecvFlags::empty())
        .map(|msg| {
            // Extract file descriptors from ancillary data.
            let mut fds = alloc::vec::Vec::new();
            for m in control.drain() {
                if let RecvAncillaryMessage::ScmRights(rights) = m {
                    fds.extend(rights);
                }
            }
            (msg.bytes, fds)
        })
        .map_err(io::Error::from)
}

/// Send a message to a Unix socket, including any file descriptors.
///
/// This is a low-level helper that performs the `sendmsg` syscall.
#[doc(hidden)]
pub fn sendmsg(fd: impl AsFd, buf: &[u8], fds: &[BorrowedFd<'_>]) -> io::Result<usize> {
    use rustix::net::{sendmsg, SendAncillaryBuffer, SendAncillaryMessage, SendFlags};
    use std::io::IoSlice;

    let mut cmsg_buf = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(MAX_FDS))];
    let mut control = SendAncillaryBuffer::new(&mut cmsg_buf);

    if !fds.is_empty() && !control.push(SendAncillaryMessage::ScmRights(fds)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "too many file descriptors to send",
        ));
    }

    let iov = [IoSlice::new(buf)];
    sendmsg(fd.as_fd(), &iov, &mut control, SendFlags::empty()).map_err(io::Error::from)
}

/// The maximum number of file descriptors that can be sent in a single message.
///
/// The value is based on what is used in `zbus`, which comes from sdbus.
const MAX_FDS: usize = 1024;
