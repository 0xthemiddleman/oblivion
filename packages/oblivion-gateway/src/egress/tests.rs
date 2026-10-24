use super::*;
use oblivion_transport::{Authentication, Metadata};
use std::net::{Ipv4Addr, SocketAddrV4};
use tokio::sync::oneshot;

const TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn configuration(endpoint: SocketAddrV4, authenticated: bool) -> Credentials {
    Credentials {
        session_token: Zeroizing::new(TOKEN.into()),
        transport: Transport::Socks5 {
            endpoint,
            authentication: authenticated.then(|| Authentication {
                username: Zeroizing::new("external-user".into()),
                password: Zeroizing::new("external-password".into()),
            }),
        },
    }
}

async fn fixture(
    authenticated: bool,
) -> (
    Arc<Egress>,
    TcpStream,
    TcpListener,
    tokio::task::JoinHandle<io::Result<()>>,
) {
    let upstream = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    // The unit fixture supplies a prevalidated internal configuration so its
    // real proxy can use a dynamic loopback port. Egress::new rejects this
    // endpoint in production; validation has independent coverage below.
    let egress = Arc::new(Egress {
        credentials: configuration(
            SocketAddrV4::new(Ipv4Addr::LOCALHOST, upstream.local_addr().unwrap().port()),
            authenticated,
        ),
        cancelled: CancellationToken::new(),
    });
    let server = tokio::spawn(egress.clone().serve(listener));
    let client = TcpStream::connect(address).await.unwrap();
    (egress, client, upstream, server)
}

async fn authenticate(client: &mut TcpStream, token: &[u8]) -> [u8; 2] {
    client.write_all(&[5, 1, 2]).await.unwrap();
    let mut reply = [0; 2];
    client.read_exact(&mut reply).await.unwrap();
    assert_eq!(reply, [5, 2]);
    let mut auth = vec![1, USER.len() as u8];
    auth.extend_from_slice(USER);
    auth.push(token.len() as u8);
    auth.extend_from_slice(token);
    client.write_all(&auth).await.unwrap();
    client.read_exact(&mut reply).await.unwrap();
    reply
}

async fn request(client: &mut TcpStream, host: &[u8], port: u16) {
    let mut bytes = vec![5, 1, 0, 3, host.len() as u8];
    bytes.extend_from_slice(host);
    bytes.extend_from_slice(&port.to_be_bytes());
    client.write_all(&bytes).await.unwrap();
}

async fn external(listener: TcpListener, authenticated: bool) -> TcpStream {
    let (mut proxy, _) = listener.accept().await.unwrap();
    let mut greeting = [0; 3];
    proxy.read_exact(&mut greeting).await.unwrap();
    assert_eq!(greeting, [5, 1, if authenticated { 2 } else { 0 }]);
    proxy.write_all(&[5, greeting[2]]).await.unwrap();
    if authenticated {
        assert_eq!(proxy.read_u8().await.unwrap(), 1);
        let mut user = vec![0; proxy.read_u8().await.unwrap() as usize];
        proxy.read_exact(&mut user).await.unwrap();
        let mut password = vec![0; proxy.read_u8().await.unwrap() as usize];
        proxy.read_exact(&mut password).await.unwrap();
        assert_eq!(user, b"external-user");
        assert_eq!(password, b"external-password");
        assert_ne!(password, TOKEN.as_bytes());
        proxy.write_all(&[1, 0]).await.unwrap();
    }
    let mut header = [0; 5];
    proxy.read_exact(&mut header).await.unwrap();
    assert_eq!(header, [5, 1, 0, 3, HOST.len() as u8]);
    let mut domain = vec![0; header[4] as usize];
    proxy.read_exact(&mut domain).await.unwrap();
    assert_eq!(domain, HOST);
    assert_eq!(proxy.read_u16().await.unwrap(), 443);
    proxy
        .write_all(&[5, 0, 0, 1, 192, 0, 2, 9, 23, 45])
        .await
        .unwrap();
    proxy
}

#[test]
fn transport_validation_and_metadata_never_expose_proxy_secrets() {
    let good = configuration("192.168.1.7:1080".parse().unwrap(), true);
    assert!(Egress::new(good).is_ok());
    assert!(matches!(Transport::default(), Transport::Tor {}));
    for endpoint in [
        "0.0.0.0:1080",
        "0.1.2.3:1080",
        "127.0.0.1:9050",
        "10.88.0.1:1080",
        "10.88.0.2:1080",
        "224.0.0.1:1080",
        "255.255.255.255:1080",
        "192.0.2.1:0",
    ] {
        assert!(Egress::new(configuration(endpoint.parse().unwrap(), false)).is_err());
    }
    for bytes in [
        r#"{"mode":"direct"}"#,
        r#"{"mode":"socks5","endpoint":"proxy.example:1080"}"#,
        r#"{"mode":"socks5","endpoint":"[::1]:1080"}"#,
        r#"{"mode":"tor","endpoint":"192.0.2.1:1080"}"#,
    ] {
        assert!(serde_json::from_str::<Transport>(bytes).is_err());
    }
    for password in [String::new(), "x".repeat(256), "unsafe\ncontrol".into()] {
        let transport = Transport::Socks5 {
            endpoint: "192.0.2.1:1080".parse().unwrap(),
            authentication: Some(Authentication {
                username: Zeroizing::new("name".into()),
                password: Zeroizing::new(password),
            }),
        };
        assert!(transport.validate().is_err());
    }
    let config = configuration("192.0.2.7:1080".parse().unwrap(), true);
    let metadata = serde_json::to_string(&config.transport.metadata()).unwrap();
    assert!(!metadata.contains("external-user") && !metadata.contains("external-password"));
    assert!(matches!(
        serde_json::from_str::<Metadata>(&metadata).unwrap(),
        Metadata::Socks5 {
            authenticated: true,
            ..
        }
    ));
}

#[tokio::test]
async fn remote_dns_and_distinct_proxy_authentication_relay_and_revoke() {
    for authenticated in [false, true] {
        timeout(Duration::from_secs(5), async {
            let (egress, mut client, upstream, server) = fixture(authenticated).await;
            assert_eq!(authenticate(&mut client, TOKEN.as_bytes()).await, [1, 0]);
            let (ready, observed) = oneshot::channel();
            let proxy = tokio::spawn(async move {
                let mut proxy = external(upstream, authenticated).await;
                let mut bytes = [0; 7];
                proxy.read_exact(&mut bytes).await.unwrap();
                assert_eq!(&bytes, b"opaque!");
                proxy.write_all(b"reply!").await.unwrap();
                ready.send(()).unwrap();
                assert_eq!(proxy.read(&mut bytes).await.unwrap(), 0);
            });
            request(&mut client, HOST, 443).await;
            let mut reply = [0; 10];
            client.read_exact(&mut reply).await.unwrap();
            assert_eq!(reply, [5, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
            client.write_all(b"opaque!").await.unwrap();
            let mut bytes = [0; 6];
            client.read_exact(&mut bytes).await.unwrap();
            assert_eq!(&bytes, b"reply!");
            observed.await.unwrap();
            egress.revoke();
            server.await.unwrap().unwrap();
            assert_eq!(client.read(&mut bytes).await.unwrap(), 0);
            proxy.await.unwrap();
        })
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn local_admission_rejects_before_any_proxy_connection() {
    use futures_util::FutureExt;
    timeout(Duration::from_secs(5), async {
        for denied in [0, 1, 2, 3, 4] {
            let (egress, mut client, upstream, server) = fixture(false).await;
            if denied == 0 {
                assert_eq!(authenticate(&mut client, b"wrong").await, [1, 1]);
            } else {
                assert_eq!(authenticate(&mut client, TOKEN.as_bytes()).await, [1, 0]);
                if denied == 1 {
                    request(&mut client, b"example.com", 443).await;
                }
                if denied == 2 {
                    request(&mut client, HOST, 80).await;
                }
                if denied == 3 {
                    client.write_all(&[5, 1, 0, 1]).await.unwrap();
                }
                if denied == 4 {
                    client.write_all(&[5, 3, 0, 3]).await.unwrap();
                }
                let mut reply = [0; 10];
                client.read_exact(&mut reply).await.unwrap();
                assert_ne!(reply[1], 0);
            }
            assert_eq!(client.read(&mut [0]).await.unwrap(), 0);
            assert!(upstream.accept().now_or_never().is_none());
            egress.revoke();
            server.await.unwrap().unwrap();
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn proxy_downgrade_bad_password_and_connect_failure_do_not_retry() {
    use futures_util::FutureExt;
    timeout(Duration::from_secs(5), async {
        for failure in [0, 1, 2] {
            let (egress, mut client, upstream, server) = fixture(true).await;
            assert_eq!(authenticate(&mut client, TOKEN.as_bytes()).await, [1, 0]);
            request(&mut client, HOST, 443).await;
            let (mut proxy, _) = upstream.accept().await.unwrap();
            let mut greeting = [0; 3];
            proxy.read_exact(&mut greeting).await.unwrap();
            assert_eq!(greeting, [5, 1, 2]);
            proxy
                .write_all(&[5, if failure == 0 { 0 } else { 2 }])
                .await
                .unwrap();
            if failure > 0 {
                assert_eq!(proxy.read_u8().await.unwrap(), 1);
                let mut bytes = vec![0; proxy.read_u8().await.unwrap() as usize];
                proxy.read_exact(&mut bytes).await.unwrap();
                let mut bytes = vec![0; proxy.read_u8().await.unwrap() as usize];
                proxy.read_exact(&mut bytes).await.unwrap();
                proxy
                    .write_all(&[1, if failure == 1 { 1 } else { 0 }])
                    .await
                    .unwrap();
                if failure == 2 {
                    let mut request = vec![0; 7 + HOST.len()];
                    proxy.read_exact(&mut request).await.unwrap();
                    proxy.write_all(&[5, 5, 0, 1]).await.unwrap();
                }
            }
            assert_eq!(client.read(&mut [0]).await.unwrap(), 0);
            assert_eq!(proxy.read(&mut [0]).await.unwrap(), 0);
            assert!(upstream.accept().now_or_never().is_none());
            egress.revoke();
            server.await.unwrap().unwrap();
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn fragmented_handshake_has_an_absolute_deadline_and_listener_recovers() {
    timeout(Duration::from_secs(15), async {
        let (egress, client, upstream, server) = fixture(false).await;
        let address = client.peer_addr().unwrap();
        let (mut incoming, mut outgoing) = client.into_split();
        // Real I/O shares the production ten-second budget. Progress at three
        // and six seconds must not restart that budget for the next field.
        let drip = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(3));
            for byte in [5, 1, 2, 1, 12] {
                interval.tick().await;
                if outgoing.write_all(&[byte]).await.is_err() {
                    return;
                }
            }
        });
        let mut reply = Vec::new();
        incoming.read_to_end(&mut reply).await.unwrap();
        assert_eq!(reply, [5, 2]);
        drip.abort();
        let _ = drip.await;
        let mut next = TcpStream::connect(address).await.unwrap();
        assert_eq!(authenticate(&mut next, b"bad").await, [1, 1]);
        use futures_util::FutureExt;
        assert!(upstream.accept().now_or_never().is_none());
        egress.revoke();
        server.await.unwrap().unwrap();
    })
    .await
    .unwrap();
}
