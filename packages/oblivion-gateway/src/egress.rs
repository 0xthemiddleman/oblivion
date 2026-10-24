use oblivion_transport::Transport;
use serde::Deserialize;
use std::{io, sync::Arc, time::Duration};
use subtle::ConstantTimeEq;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, copy_bidirectional},
    net::{TcpListener, TcpStream},
    task::JoinSet,
    time::timeout,
};
use tokio_util::sync::CancellationToken;
use zeroize::Zeroizing;

const USER: &[u8] = b"oblivion-api";
const HOST: &[u8] = b"api.abliteration.ai";
const HANDSHAKE: Duration = Duration::from_secs(10);

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Credentials {
    pub transport: Transport,
    pub session_token: Zeroizing<String>,
}

// One connector per grant. This first policy admits only the fixed API origin;
// the future tools proxy must authorize its own destinations before adding a
// separate channel. This is not a general SOCKS listener exposed to the guest.
pub struct Egress {
    credentials: Credentials,
    cancelled: CancellationToken,
}

impl Egress {
    pub fn new(credentials: Credentials) -> Result<Self, &'static str> {
        credentials.transport.validate()?;
        if credentials.session_token.len() != 64
            || !credentials
                .session_token
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err("invalid_session_token");
        }
        Ok(Self {
            credentials,
            cancelled: CancellationToken::new(),
        })
    }

    pub fn revoke(&self) {
        self.cancelled.cancel();
    }

    pub async fn serve(self: Arc<Self>, listener: TcpListener) -> io::Result<()> {
        if !listener.local_addr()?.ip().is_loopback() {
            return Err(io::Error::other("loopback_required"));
        }
        let mut tasks = JoinSet::new();
        let result = loop {
            tokio::select! {
                biased;
                _ = self.cancelled.cancelled() => break Ok(()),
                _ = tasks.join_next(), if !tasks.is_empty() => {},
                incoming = listener.accept() => {
                    let (mut client, _) = match incoming {
                        Ok(value) => value,
                        Err(error) => break Err(error),
                    };
                    if tasks.len() >= 64 { continue; }
                    let egress = self.clone();
                    tasks.spawn(async move {
                        // One absolute budget covers fragmented local input,
                        // proxy connect and authentication, with no retry.
                        let Ok(Ok(mut upstream)) = timeout(
                            HANDSHAKE,
                            egress.handshake(&mut client),
                        ).await else { return; };
                        let _ = copy_bidirectional(&mut client, &mut upstream).await;
                    });
                },
            }
        };
        drop(listener);
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
        result
    }

    async fn handshake(&self, client: &mut TcpStream) -> io::Result<TcpStream> {
        let mut greeting = [0; 2];
        client.read_exact(&mut greeting).await?;
        if greeting[0] != 5 || greeting[1] == 0 {
            return Err(io::Error::other("invalid_socks_greeting"));
        }
        let mut methods = vec![0; greeting[1] as usize];
        client.read_exact(&mut methods).await?;
        if !methods.contains(&2) {
            client.write_all(&[5, 255]).await?;
            return Err(io::Error::other("authentication_required"));
        }
        client.write_all(&[5, 2]).await?;
        let mut auth = [0; 2];
        client.read_exact(&mut auth).await?;
        if auth[0] != 1 || auth[1] == 0 {
            return Err(io::Error::other("invalid_socks_authentication"));
        }
        let mut username = Zeroizing::new(vec![0; auth[1] as usize]);
        client.read_exact(&mut username).await?;
        let size = client.read_u8().await? as usize;
        let mut password = Zeroizing::new(vec![0; size]);
        client.read_exact(&mut password).await?;
        if username.as_slice() != USER
            || !bool::from(
                password
                    .as_slice()
                    .ct_eq(self.credentials.session_token.as_bytes()),
            )
        {
            client.write_all(&[1, 1]).await?;
            return Err(io::Error::other("authentication_failed"));
        }
        drop(username);
        drop(password);
        client.write_all(&[1, 0]).await?;
        let mut request = [0; 4];
        client.read_exact(&mut request).await?;
        if request != [5, 1, 0, 3] {
            client.write_all(&[5, 7, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
            return Err(io::Error::other("request_denied"));
        }
        let size = client.read_u8().await? as usize;
        let mut domain = vec![0; size];
        client.read_exact(&mut domain).await?;
        let port = client.read_u16().await?;
        if domain != HOST || port != 443 {
            client.write_all(&[5, 2, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
            return Err(io::Error::other("destination_denied"));
        }
        // All local admission precedes even connecting to the upstream proxy.
        let mut upstream = TcpStream::connect(self.credentials.transport.endpoint()).await?;
        let authentication = match &self.credentials.transport {
            Transport::Tor {} => Some((USER, self.credentials.session_token.as_bytes())),
            Transport::Socks5 { authentication, .. } => authentication
                .as_ref()
                .map(|auth| (auth.username.as_bytes(), auth.password.as_bytes())),
        };
        let method = if authentication.is_some() { 2 } else { 0 };
        upstream.write_all(&[5, 1, method]).await?;
        let mut selected = [0; 2];
        upstream.read_exact(&mut selected).await?;
        if selected != [5, method] {
            return Err(io::Error::other("proxy_authentication_method_rejected"));
        }
        if let Some((username, password)) = authentication {
            let mut bytes = Zeroizing::new(Vec::with_capacity(3 + username.len() + password.len()));
            bytes.extend_from_slice(&[1, username.len() as u8]);
            bytes.extend_from_slice(username);
            bytes.push(password.len() as u8);
            bytes.extend_from_slice(password);
            upstream.write_all(&bytes).await?;
            drop(bytes);
            let mut reply = [0; 2];
            upstream.read_exact(&mut reply).await?;
            if reply != [1, 0] {
                return Err(io::Error::other("proxy_authentication_failed"));
            }
        }
        let mut request = vec![5, 1, 0, 3, HOST.len() as u8];
        request.extend_from_slice(HOST);
        request.extend_from_slice(&443u16.to_be_bytes());
        upstream.write_all(&request).await?;
        let mut reply = [0; 4];
        upstream.read_exact(&mut reply).await?;
        if reply[..3] != [5, 0, 0] {
            return Err(io::Error::other("proxy_connect_failed"));
        }
        // BND.ADDR is the proxy's bound endpoint, not evidence of the resolved
        // destination. Never resolve the requested domain locally or retry it.
        let size = match reply[3] {
            1 => 4,
            3 => {
                let size = upstream.read_u8().await? as usize;
                if size == 0 {
                    return Err(io::Error::other("invalid_proxy_reply"));
                }
                size
            }
            4 => 16,
            _ => return Err(io::Error::other("invalid_proxy_reply")),
        };
        let mut bound = vec![0; size + 2];
        upstream.read_exact(&mut bound).await?;
        client.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).await?;
        Ok(upstream)
    }
}

#[cfg(test)]
mod tests;
