//! Integration test for peer credentials functionality.

use std::os::fd::{AsFd, AsRawFd};
use tempfile::TempDir;
use zlink::Listener;

#[tokio::test]
async fn peer_credentials_unix_socket() {
    // Get the expected credentials of the current process.
    let expected_uid = rustix::process::getuid();
    let expected_pid = rustix::process::getpid();

    // Create a temporary directory for the socket.
    let temp_dir = TempDir::new().unwrap();
    let socket_path = temp_dir.path().join("test_creds.sock");

    // Create a listener.
    let mut listener = zlink::unix::bind(&socket_path).unwrap();

    // Connect from a client.
    let socket_path_clone = socket_path.clone();
    let connect_task = tokio::spawn(async move {
        tokio::net::UnixStream::connect(&socket_path_clone)
            .await
            .unwrap()
    });

    let mut connection = listener.accept().await.unwrap();

    // Get peer credentials.
    let creds = connection.peer_credentials().await.unwrap();

    // Verify we got the correct credentials.
    assert_eq!(
        creds.unix_user_id(),
        expected_uid,
        "UID should match current process"
    );

    // On most platforms, we can get the actual PID.
    // Exception: FreeBSD < 13 and DragonFly BSD (where it's 0).
    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "macos",
        target_os = "ios",
        target_os = "openbsd",
        target_os = "netbsd"
    ))]
    assert_eq!(
        creds.process_id(),
        expected_pid,
        "PID should match current process"
    );

    #[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
    {
        // FreeBSD 13+ has PID support, older versions return 0.
        // DragonFly BSD currently returns 0 (PID support TBD).
        let pid = creds.process_id();
        assert!(
            pid == expected_pid || pid == rustix::process::Pid::from_raw(0).unwrap(),
            "PID should be either the actual PID or 0 on older FreeBSD/DragonFly"
        );
    }

    // On Linux, we expect a valid process FD (pidfd) on recent kernels.
    #[cfg(target_os = "linux")]
    {
        let pidfd = creds.process_fd();

        // Verify the pidfd is valid by checking if we can use it.
        let fd_num = pidfd.as_fd().as_raw_fd();
        assert!(fd_num >= 0, "Process FD should be valid");

        // Verify the pidfd refers to the correct process by reading /proc/self/fdinfo.
        let fdinfo_path = format!("/proc/self/fdinfo/{}", fd_num);
        let fdinfo = std::fs::read_to_string(&fdinfo_path).unwrap();
        // The fdinfo should contain "Pid:" followed by our PID.

        use rustix::process::Pid;
        let pid_line = fdinfo
            .lines()
            .find(|line| line.starts_with("Pid:"))
            .expect("fdinfo should contain Pid field");
        let pid = pid_line
            .strip_prefix("Pid:")
            .unwrap()
            .trim()
            .parse::<i32>()
            .expect("Pid field should be a valid number");
        let pid = Pid::from_raw(pid).unwrap();
        assert_eq!(
            pid, expected_pid,
            "pidfd should refer to the correct process"
        );
    }

    // Save values for comparison.
    let uid1 = creds.unix_user_id();
    let pid1 = creds.process_id();
    #[cfg(target_os = "linux")]
    let pidfd1 = creds.process_fd().as_raw_fd();

    // Verify caching works - calling again should return the same values.
    let creds2 = connection.peer_credentials().await.unwrap();
    assert_eq!(uid1, creds2.unix_user_id(), "Cached UID should match");
    assert_eq!(pid1, creds2.process_id(), "Cached PID should match");
    #[cfg(target_os = "linux")]
    assert_eq!(
        pidfd1,
        creds2.process_fd().as_raw_fd(),
        "Cached pidfd should match"
    );

    let _stream = connect_task.await.unwrap();
}
