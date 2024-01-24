use crate::prelude::*;
use core::{future::Future, pin::Pin};

#[cfg(any(test, feature = "tokio"))]
pub mod tokio;

/// Provides a means for executing Git CLI commands.
///
/// There is no `arg0` passed; it's up to the implementation
/// to decide how to execute the command. For example,
/// `git status` would be passed as `["status"]`.
///
/// The executor also provides a means for spinning up
/// ad-hoc socket servers, necessary for the authorization
/// utilities that are passed to Git and SSH to communicate
/// with the host process to exchange credentials. We also
/// implement a simple layer of security over this communication
/// layer to avoid unintended leakage of credentials.
///
/// Note that this security layer is _not_ impervious
/// to determined attackers. It is merely a means to
/// avoid unintended connections to the socket server
/// or simple, generic attacks. The threat model assumes
/// that more sophisticated attacks targeting the host system
/// are out of scope for this project given that the
/// communication layer is not a far cry from the user
/// inputting the credentials manually, directly into the
/// CLI utility.
///
/// # Safety
///
/// This trait is marked as unsafe due to the platform-specific
/// invariants described in [`GitExecutor::create_askpass_server`].
/// These invariants are not enforced by the typesystem, and while
/// we have some loose checks to ensure that the invariants are upheld,
/// we cannot guarantee that they are upheld in all cases. Thus, it is
/// up to the implementor to ensure that the invariants are upheld.
#[allow(unsafe_code)]
pub unsafe trait GitExecutor {
    /// The error type returned by this executor,
    /// specifically in cases where the execution fails.
    ///
    /// Otherwise, `Ok` is returned in call cases, even when
    /// the exit code is non-zero.
    type Error: core::error::Error + core::fmt::Debug + Send + Sync + 'static;

    /// The type of the handle returned by [`GitExecutor::create_askpass_server`].
    type ServerHandle: AskpassServer + Send + Sync + 'static;

    /// The type of socket passed to the connection handler
    /// provided to [`GitExecutor::create_askpass_server`].
    type SocketHandle: Socket + Send + Sync + 'static;

    /// Executes the given Git command with the given arguments.
    /// `git` is never passed as the first argument (arg 0).
    ///
    /// Returns a tuple of `(exit_code, stdout, stderr)`.
    ///
    /// `Err` is returned if the command could not be executed,
    /// **not** if the command returned a non-zero exit code.
    async fn execute_raw(
        &self,
        args: &[&str],
        envs: Option<BTreeMap<String, String>>,
    ) -> Result<(usize, String, String), Self::Error>;

    /// Executes the given Git command with sane defaults.
    /// `git` is never passed as the first argument (arg 0).
    ///
    /// Implementers should use this method over [`Self::execute_raw`]
    /// when possible.
    async fn execute(
        &self,
        args: &[&str],
        envs: Option<BTreeMap<String, String>>,
    ) -> Result<(usize, String, String), Self::Error> {
        let mut args = args.as_ref().to_vec();

        args.insert(0, "--no-pager");
        // TODO(qix-): Test the performance impact of this.
        args.insert(0, "--no-optional-locks");

        let mut envs = envs.unwrap_or_default();
        envs.insert("GIT_TERMINAL_PROMPT".into(), "0".into());
        envs.insert("LC_ALL".into(), "C".into()); // Force English. We need this for parsing output.

        self.execute_raw(&args, Some(envs)).await
    }

    /// Creates a named pipe server that is compatible with
    /// the `askpass` utility (see `bin/askpass.rs` and platform-specific
    /// adjacent sources).
    ///
    /// Servers created should be non-blocking and exist in either
    /// another thread or an async task; the method should return
    /// immediately with a handle to the server, as described below.
    ///
    /// ## Unix
    ///
    /// On Unix-like systems (including MacOS), this is a unix
    /// domain socket. The path of the socket is returned as
    /// a handle type that is format-able as a string which is
    /// passed to the askpass utility as `GITBUTLER_ASKPASS_SOCKET`.
    ///
    /// The socket itself should be created as read/write for the user
    /// with no access to group or everyone (`0600` or `u+rw ag-a`).
    ///
    /// Further, the callback is invoked with the credentials for
    /// the socket upon a connection and is awaited. After the callback
    /// returns, the connection must be closed.
    ///
    /// Upon the handle being dropped, the socket must be closed and
    /// the socket file SHOULD be best-effort unlinked.
    ///
    /// Given that this invariant must be upheld, this method is marked
    /// as unsafe.
    ///
    /// ## Windows
    ///
    /// On Windows, this is a named pipe. The handle returned must be
    /// format-able as a string which is passed to the askpass utility
    /// as `GITBUTLER_ASKPASS_SOCKET` and corresponds to the named
    /// pipe.
    ///
    /// The pipe name MUST start with `\.\pipe\LOCAL\`. Given that this
    /// invariant must be upheld, this method is marked as unsafe.
    ///
    /// The callback is invoked with the process ID of the client
    /// upon a connection, and is awaited. After the callback returns,
    /// the connection must be closed.
    ///
    /// Upon the handle being dropped, the pipe must be closed.
    ///
    /// # Safety
    ///
    /// This method is marked as unsafe due to the platform-specific
    /// invariants described above that must be upheld by all implementations.
    /// These invariants are not enforced by the typesystem, and while
    /// we have some loose checks to ensure that the invariants are upheld,
    /// we cannot guarantee that they are upheld in all cases. Thus, it is
    /// up to the implementor to ensure that the invariants are upheld.
    ///
    /// If for some reason these invariants are not possible to uphold,
    /// please open an issue on the repository to discuss this issue.
    unsafe fn create_askpass_server<F>(
        &self,
        callback: F,
    ) -> Result<Self::ServerHandle, Self::Error>
    where
        F: Fn(
                Self::SocketHandle,
            ) -> Pin<
                Box<
                    dyn Future<Output = Result<(), <Self as GitExecutor>::SocketHandle>>
                        + Send
                        + 'static,
                >,
            > + Send
            + 'static;
}

/// A handle to a server created by [`GitExecutor::create_askpass_server`].
///
/// When formatted as a string, the result should be the connection string
/// necessary for the askpass utility to connect (e.g. a unix domain socket path
/// or a windows named pipe name; see [`GitExecutor::create_askpass_server`] for
/// more information).
///
/// Upon dropping the handle, the server should be closed.
pub trait AskpassServer: Drop + core::fmt::Display {}

#[cfg(unix)]
type PidInner = ::nix::unistd::Pid;

#[cfg(windows)]
type PidInner = u32;

/// The type of a process ID (platforms-specific)
pub type Pid = PidInner;

/// The type of a user ID (unix-specific).
#[cfg(unix)]
pub type Uid = ::nix::unistd::Uid;

/// Platform-specific credentials for a connection to a server created by
/// [`GitExecutor::create_askpass_server`]. This is passed to the callback
/// provided to [`GitExecutor::create_askpass_server`] when a connection
/// is established.
pub trait Socket {
    /// The error type returned by I/O operations on this socket.
    type Error: core::error::Error + core::fmt::Debug + Send + Sync + 'static;

    /// The process ID of the connecting client.
    fn pid(&self) -> Result<Pid, Self::Error>;

    /// The user ID of the connecting client.
    #[cfg(unix)]
    fn uid(&self) -> Result<Uid, Self::Error>;

    /// Reads a line from the socket. Must not include the newline.
    ///
    /// The returned line must not include a newline, and any
    /// trailing carriage return (`\r`) must be stripped.
    ///
    /// Implementations are allowed to simply call `.trim()` on the
    /// line, as whitespace is not significant in the protocol.
    async fn read_line(&mut self) -> Result<String, Self::Error>;

    /// Writes a line to the socket. The write must
    /// complete fully before returning (i.e. implementations
    /// should use something akin to `write_all`).
    ///
    /// The input line will not include a newline; one must be
    /// added. Newlines should never include a carriage return (`\r`).
    ///
    /// Unlike `read_line`, implementations are not allowed to
    /// modify the line prior to sending aside from appending a newline.
    async fn write_line(&mut self, line: &str) -> Result<(), Self::Error>;
}
