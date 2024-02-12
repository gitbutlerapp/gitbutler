use futures::FutureExt;
use russh::{server, Channel, ChannelId, MethodSet, Pty};
use std::{collections::HashMap, process::Stdio, sync::Arc};
use tokio::net::TcpListener;

#[derive(Debug)]
pub(crate) struct TestSshServer {
    repo_path: String,
    allowed_auths: Vec<crate::Authorization>,
}

impl TestSshServer {
    pub fn new(repo_path: String) -> Self {
        Self {
            repo_path,
            allowed_auths: Vec::new(),
        }
    }

    pub async fn run_with_server<F, FN>(self, cb: FN)
    where
        FN: FnOnce(u16) -> F,
        F: std::future::Future<Output = ()> + 'static,
    {
        // We manually set up a TcpListener here so that we can
        // bind to a random port and retrieve it.
        let listener = TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let port = addr.port();

        let config = Arc::new(russh::server::Config {
            inactivity_timeout: Some(std::time::Duration::from_secs(10)),
            auth_rejection_time: std::time::Duration::from_secs(3),
            auth_rejection_time_initial: Some(std::time::Duration::from_secs(0)),
            keys: vec![russh_keys::key::KeyPair::generate_ed25519().unwrap()],
            ..Default::default()
        });

        let socket_future = russh::server::run_on_socket(config, &listener, self);

        futures::select! {
            _ = cb(port).fuse() => {},
            _ = socket_future.fuse() => {
                panic!("server exited prematurely");
            },
        }
    }

    #[allow(unused)]
    pub fn allow_authorization(&mut self, auth: crate::Authorization) {
        self.allowed_auths.push(auth);
    }
}

impl server::Server for TestSshServer {
    type Handler = TestSshClient;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self::Handler {
        TestSshClient {
            repo_path: self.repo_path.clone(),
            channels: HashMap::new(),
            allowed_auths: self.allowed_auths.clone(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct TestSshClient {
    repo_path: String,
    channels: HashMap<ChannelId, TestSshChannel>,
    allowed_auths: Vec<crate::Authorization>,
}

#[derive(Debug)]
struct TestSshChannel {
    envs: HashMap<String, String>,
    channel: Channel<server::Msg>,
}

#[async_trait::async_trait]
impl server::Handler for TestSshClient {
    type Error = russh::Error;

    async fn auth_password(
        self,
        user: &str,
        pass: &str,
    ) -> Result<(Self, server::Auth), Self::Error> {
        for auth in &self.allowed_auths {
            if let crate::Authorization::Basic { username, password } = auth {
                if username.as_deref() == Some(user) && password.as_deref() == Some(pass) {
                    return Ok((self, server::Auth::Accept));
                }
            }
        }

        Ok((
            self,
            server::Auth::Reject {
                proceed_with_methods: Some(MethodSet::PUBLICKEY),
            },
        ))
    }

    async fn env_request(
        mut self,
        channel: ChannelId,
        name: &str,
        value: &str,
        session: server::Session,
    ) -> Result<(Self, server::Session), Self::Error> {
        match name {
            name if name.starts_with("LC_") || name == "GIT_PROTOCOL" || name == "LANG" => {
                self.channels
                    .get_mut(&channel)
                    .expect("env_request on unknown channel")
                    .envs
                    .insert(name.to_owned(), value.to_owned());
            }
            disallowed => {
                eprintln!(
                    "client attempted to set disallowed environment variable {:?} to {:?}",
                    disallowed, value
                )
            }
        }
        Ok((self, session))
    }

    async fn pty_request(
        self,
        _channel: ChannelId,
        _term: &str,
        _col_width: u32,
        _row_height: u32,
        _pix_width: u32,
        _pix_height: u32,
        _modes: &[(Pty, u32)],
        _session: server::Session,
    ) -> Result<(Self, server::Session), Self::Error> {
        panic!("client requested a pty but we don't support that");
    }

    async fn shell_request(
        self,
        _channel: ChannelId,
        _session: server::Session,
    ) -> Result<(Self, server::Session), Self::Error> {
        panic!("client requested a shell but we don't support that");
    }

    async fn exec_request(
        mut self,
        channel_id: ChannelId,
        command: &[u8],
        session: server::Session,
    ) -> Result<(Self, server::Session), Self::Error> {
        let req = String::from_utf8_lossy(command);

        if req.starts_with("git-upload-pack") {
            let channel = Box::leak(Box::new(self.channels.remove(&channel_id).unwrap()));
            let repo_path = self.repo_path.clone();
            let handle = session.handle();

            tokio::spawn(async move {
                let channel_id = channel.channel.id();
                let mut writer = channel.channel.make_writer_ext(None);
                let mut reader = channel.channel.make_reader_ext(None);

                let mut cmd = tokio::process::Command::new("git-upload-pack")
                    .kill_on_drop(true)
                    .envs(channel.envs.iter())
                    .arg(&repo_path)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .spawn()
                    .unwrap();

                let mut stdin = cmd.stdin.take().unwrap();
                let mut stdout = cmd.stdout.take().unwrap();

                let copy_in = tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};

                    //let file = tokio::fs::File::create("/tmp/gitbutler-upload-pack-in.log")
                    //    .await
                    //    .unwrap();
                    //let mut file_writer = tokio::io::BufWriter::new(file);

                    let mut buffer = [0; 1024];
                    while let Ok(n) = reader.read(&mut buffer).await {
                        if n == 0 {
                            break;
                        }
                        stdin.write_all(&buffer[..n]).await.unwrap();
                        //file_writer.write_all(&buffer[..n]).await.unwrap();
                        stdin.flush().await.unwrap();
                        //file_writer.flush().await.unwrap();
                    }

                    stdin.shutdown().await.ok(); // may have already been closed
                                                 //file_writer.shutdown().await.unwrap();
                });

                let copy_out = tokio::spawn(async move {
                    use tokio::io::{AsyncReadExt, AsyncWriteExt};

                    //let file = tokio::fs::File::create("/tmp/gitbutler-upload-pack-out.log")
                    //    .await
                    //    .unwrap();
                    //let mut file_writer = tokio::io::BufWriter::new(file);

                    let mut buffer = [0; 1024];
                    while let Ok(n) = stdout.read(&mut buffer).await {
                        if n == 0 {
                            break;
                        }

                        writer.write_all(&buffer[..n]).await.unwrap();
                        //file_writer.write_all(&buffer[..n]).await.unwrap();
                        writer.flush().await.unwrap();
                        //file_writer.flush().await.unwrap();
                    }
                    writer.shutdown().await.ok(); // may have already been closed.
                                                  //file_writer.shutdown().await.unwrap();
                });

                let cmd_future = tokio::spawn(async move { cmd.wait().await.unwrap() });

                let (status, _, _) = futures::try_join!(cmd_future, copy_in, copy_out).unwrap();

                let exit_code = status.code().unwrap_or(1) as u32;

                handle
                    .exit_status_request(channel_id, exit_code)
                    .await
                    .unwrap();

                handle.close(channel_id).await.unwrap();
            });
        } else {
            panic!("client requested a command we don't support: {:?}", req);
        }

        Ok((self, session))
    }

    async fn channel_open_session(
        mut self,
        channel: Channel<server::Msg>,
        session: server::Session,
    ) -> Result<(Self, bool, server::Session), Self::Error> {
        self.channels.insert(
            channel.id(),
            TestSshChannel {
                channel,
                envs: HashMap::new(),
            },
        );
        Ok((self, true, session))
    }

    async fn channel_close(
        mut self,
        channel: ChannelId,
        session: server::Session,
    ) -> Result<(Self, server::Session), Self::Error> {
        // Best effort; may already be consumed.
        self.channels.remove(&channel);
        Ok((self, session))
    }
}

#[allow(unused_macros)]
macro_rules! test_impl {
    ($create_repo:expr, enable_io, $(async fn $name:ident($repo:ident $(, $server:ident , $server_repo:ident)?) { $($body:tt)* })*) => {
        $($crate::private::test_impl!($create_repo, $name, $repo $(, $server, $server_repo)?, { $($body)* });)*
    };
    ($create_repo:expr, disable_io, $(async fn $name:ident($repo:ident $(, $server:ident , $server_repo:ident)?) { $($body:tt)* })*) => {};
    ($create_repo:expr, $name:ident, $repo:ident, { $($body:tt)* }) => {
        #[test]
        fn $name() {
            ::tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                #[allow(unused_variables)]
                let $repo = $create_repo({
                    let mod_name = ::std::module_path!();
                    let test_name = ::std::stringify!($name);
                    format!("{mod_name}::{test_name}")
                }).await;

                let test_future = async { $($body)* };

                use futures::FutureExt;
                let timeout_future = ::tokio::time::sleep(::std::time::Duration::from_secs(10));

                futures::select! {
                    _ = test_future.fuse() => {},
                    _ = timeout_future.fuse() => {
                        panic!("test timed out");
                    },
                }
            })
        }
    };
    ($create_repo:expr, $name:ident, $repo:ident, $server:ident, $server_repo:ident, { $($body:tt)* }) => {
        #[test]
        fn $name() {
            ::tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                #[allow(unused_variables)]
                let $repo = $create_repo({
                    let mod_name = ::std::module_path!();
                    let test_name = ::std::stringify!($name);
                    format!("{mod_name}::{test_name}")
                }).await;

                #[allow(unused_variables, unused_mut)]
                let (mut $server, $server_repo) = async {
                    let mod_name = ::std::module_path!();
                    let test_name = ::std::stringify!($name);
                    let repo_path = ::std::env::temp_dir()
                        .join("gitbutler-tests")
                        .join("git")
                        .join("remote")
                        .join(test_name)
                        .to_string_lossy()
                        .into_owned();

                    ::std::fs::create_dir_all(&repo_path).unwrap();

                    let repo = $crate::backend::git2::Repository::<
                            $crate::backend::git2::tokio::TokioThreadedResource
                        >::open_or_init_bare(repo_path.clone()).await.unwrap();

                    let server = $crate::private::TestSshServer::new(repo_path);

                    (server, repo)
                }.await;

                let test_future = async { $($body)* };

                use futures::FutureExt;
                let timeout_future = ::tokio::time::sleep(::std::time::Duration::from_secs(10));

                futures::select! {
                    _ = test_future.fuse() => {},
                    _ = timeout_future.fuse() => {
                        panic!("test timed out");
                    },
                }
            })
        }
    };
}

pub(crate) use test_impl;
