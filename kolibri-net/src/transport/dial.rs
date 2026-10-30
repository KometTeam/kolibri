//! Outbound connects. A name resolves to every address it has (see
//! [`super::resolve`]) and they are raced, Happy Eyeballs style (RFC 8305):
//! one address that drops packets no longer eats the whole connect timeout
//! while the others sit untried. For TLS the handshake is part of each attempt,
//! so an address that accepts TCP but is not the host (a stale cache entry, a
//! hijacked DNS answer) loses the race instead of winning it.

use std::future::Future;
use std::io;
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use rustls::pki_types::ServerName;
use tokio::net::TcpStream;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tokio_rustls::client::TlsStream;
use tokio_rustls::TlsConnector;

use super::doh;
use super::proxy::{self, ProxyConfig};
use super::resolve::{self, LOOKUP_TIMEOUT, LOOKUP_TIMEOUT_CACHED};

/// Head start of each attempt over the next one (RFC 8305's recommended
/// Connection Attempt Delay). A failed attempt starts the next one at once.
pub(crate) const ATTEMPT_DELAY: Duration = Duration::from_millis(250);

/// A SYN still unanswered by then (the OS has sent it three times) is being
/// dropped: the address fails, so a race that has nothing else left can move
/// on to DoH within the connect timeout instead of waiting it out.
const TCP_ATTEMPT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub(crate) enum DialError {
    /// the whole connect, resolution included, ran out of time
    TimedOut,
    /// the name did not resolve, or no address could be reached
    Io(io::Error),
    /// an address was reached, but the TLS handshake failed
    Tls(io::Error),
}

impl From<DialError> for io::Error {
    fn from(e: DialError) -> Self {
        match e {
            DialError::TimedOut => io::Error::new(io::ErrorKind::TimedOut, "connect timed out"),
            DialError::Io(e) | DialError::Tls(e) => e,
        }
    }
}

/// Open a TCP stream to `(host, port)`, directly or through `proxy`.
/// `connect_timeout` covers the lot: resolution, the race, the proxy handshake.
pub async fn connect_tcp(
    host: &str,
    port: u16,
    connect_timeout: Duration,
    proxy: Option<&ProxyConfig>,
) -> io::Result<TcpStream> {
    timeout(connect_timeout, connect_tcp_inner(host, port, proxy))
        .await
        .unwrap_or(Err(DialError::TimedOut))
        .map_err(io::Error::from)
}

async fn connect_tcp_inner(
    host: &str,
    port: u16,
    proxy: Option<&ProxyConfig>,
) -> Result<TcpStream, DialError> {
    match proxy {
        None => dial(host, port, tcp_attempt).await,
        Some(p) => {
            let mut tcp = dial(&p.host, p.port, tcp_attempt).await?;
            proxy::handshake(&mut tcp, host, port, p)
                .await
                .map_err(DialError::Io)?;
            Ok(tcp)
        }
    }
}

/// Open a TLS stream to `(host, port)` verified as `name`. Without a proxy the
/// host's addresses race through the handshake; through a proxy the proxy
/// resolves the host and there is a single handshake.
pub(crate) async fn connect_tls(
    host: &str,
    port: u16,
    connect_timeout: Duration,
    proxy: Option<&ProxyConfig>,
    connector: &TlsConnector,
    name: ServerName<'static>,
) -> Result<TlsStream<TcpStream>, DialError> {
    let connect = async {
        if proxy.is_some() {
            let tcp = connect_tcp_inner(host, port, proxy).await?;
            return connector.connect(name, tcp).await.map_err(DialError::Tls);
        }
        let attempt = |addr| {
            let connector = connector.clone();
            let name = name.clone();
            async move {
                let tcp = tcp_attempt(addr).await?;
                connector.connect(name, tcp).await.map_err(DialError::Tls)
            }
        };
        dial(host, port, attempt).await
    };
    timeout(connect_timeout, connect)
        .await
        .unwrap_or(Err(DialError::TimedOut))
}

async fn tcp_attempt(addr: SocketAddr) -> Result<TcpStream, DialError> {
    let tcp = timeout(TCP_ATTEMPT_TIMEOUT, TcpStream::connect(addr))
        .await
        .map_err(|_| DialError::Io(io::Error::new(io::ErrorKind::TimedOut, "no answer to SYN")))?
        .map_err(DialError::Io)?;
    tcp.set_nodelay(true).ok();
    Ok(tcp)
}

/// Resolve `host` and race its addresses. DoH gets a turn when the system
/// resolver and the cache come up empty, and again when none of their
/// addresses worked. The winner is remembered for the next connect.
async fn dial<T, F, Fut>(host: &str, port: u16, attempt: F) -> Result<T, DialError>
where
    F: Fn(SocketAddr) -> Fut,
    Fut: Future<Output = Result<T, DialError>> + Send + 'static,
    T: Send + 'static,
{
    if let Some(ip) = resolve::ip_literal(host) {
        return attempt(SocketAddr::new(ip, port)).await;
    }
    if resolve::is_localhost(host) {
        let addrs = resolve::system_lookup(host, port, LOOKUP_TIMEOUT)
            .await
            .map_err(DialError::Io)?;
        let outcome = race(&socket_addrs(&addrs, port), ATTEMPT_DELAY, attempt).await;
        return match outcome.won {
            Some((_, stream)) => Ok(stream),
            None => Err(summarize(host, port, outcome.failures)),
        };
    }

    let cached = resolve::cached(host);
    let limit = if cached.is_empty() {
        LOOKUP_TIMEOUT
    } else {
        LOOKUP_TIMEOUT_CACHED
    };
    let lookup = resolve::system_lookup(host, port, limit).await;
    let mut seen = lookup.as_ref().map(Vec::clone).unwrap_or_default();
    let mut candidates = resolve::order(&seen, &cached);
    let mut asked_doh = false;
    if candidates.is_empty() {
        asked_doh = true;
        seen = doh::lookup(host).await;
        candidates = seen.clone();
    }
    if candidates.is_empty() {
        return Err(DialError::Io(lookup.err().unwrap_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("{host} resolves to no usable address"),
            )
        })));
    }

    let mut failures = Vec::new();
    loop {
        let outcome = race(&socket_addrs(&candidates, port), ATTEMPT_DELAY, &attempt).await;
        failures.extend(outcome.failures);
        if let Some((winner, stream)) = outcome.won {
            resolve::remember(host, winner.ip(), &seen, &impostors(&failures));
            return Ok(stream);
        }
        if asked_doh {
            return Err(summarize(host, port, failures));
        }
        asked_doh = true;
        seen = doh::lookup(host).await;
        candidates = seen
            .iter()
            .copied()
            .filter(|ip| !failures.iter().any(|(addr, _)| addr.ip() == *ip))
            .collect();
        if candidates.is_empty() {
            return Err(summarize(host, port, failures));
        }
    }
}

/// Outcome of [`race`]: the first attempt that succeeded, and every attempt
/// that failed before it, in the order they failed.
pub(crate) struct Race<T, E> {
    pub won: Option<(SocketAddr, T)>,
    pub failures: Vec<(SocketAddr, E)>,
}

/// Start with the first address, add the next one every `delay` or as soon as
/// a running attempt fails, keep the first success and drop the rest.
pub(crate) async fn race<T, E, F, Fut>(
    addrs: &[SocketAddr],
    delay: Duration,
    attempt: F,
) -> Race<T, E>
where
    F: Fn(SocketAddr) -> Fut,
    Fut: Future<Output = Result<T, E>> + Send + 'static,
    T: Send + 'static,
    E: Send + 'static,
{
    let mut running = JoinSet::new();
    let mut pending = addrs.iter().copied();
    let mut failures = Vec::new();
    loop {
        if let Some(addr) = pending.next() {
            let fut = attempt(addr);
            running.spawn(async move { (addr, fut.await) });
        }
        if running.is_empty() {
            return Race {
                won: None,
                failures,
            };
        }
        let more = pending.len() > 0;
        tokio::select! {
            joined = running.join_next() => match joined {
                Some(Ok((addr, Ok(value)))) => {
                    return Race {
                        won: Some((addr, value)),
                        failures,
                    }
                }
                Some(Ok((addr, Err(e)))) => failures.push((addr, e)),
                Some(Err(_)) | None => {}
            },
            _ = tokio::time::sleep(delay), if more => {}
        }
    }
}

fn socket_addrs(ips: &[IpAddr], port: u16) -> Vec<SocketAddr> {
    ips.iter().map(|ip| SocketAddr::new(*ip, port)).collect()
}

/// Addresses whose certificate was rejected: something else answers there.
fn impostors(failures: &[(SocketAddr, DialError)]) -> Vec<IpAddr> {
    failures
        .iter()
        .filter(|(_, e)| matches!(e, DialError::Tls(e) if e.kind() == io::ErrorKind::InvalidData))
        .map(|(addr, _)| addr.ip())
        .collect()
}

/// One error for a failed race: the first TLS failure as is (a certificate
/// problem is what the caller has to act on), otherwise every address with
/// its error.
fn summarize(host: &str, port: u16, mut failures: Vec<(SocketAddr, DialError)>) -> DialError {
    if let Some(i) = failures
        .iter()
        .position(|(_, e)| matches!(e, DialError::Tls(_)))
    {
        return failures.swap_remove(i).1;
    }
    let kind = failures
        .last()
        .map(|(_, e)| match e {
            DialError::Io(e) => e.kind(),
            _ => io::ErrorKind::Other,
        })
        .unwrap_or(io::ErrorKind::NotFound);
    let detail = failures
        .iter()
        .map(|(addr, e)| format!("{}: {}", addr.ip(), io_error(e)))
        .collect::<Vec<_>>()
        .join("; ");
    DialError::Io(io::Error::new(
        kind,
        format!("connect to {host}:{port} failed ({detail})"),
    ))
}

fn io_error(e: &DialError) -> String {
    match e {
        DialError::TimedOut => "timed out".to_string(),
        DialError::Io(e) | DialError::Tls(e) => e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
    use tokio::net::TcpListener;
    use tokio::time::{sleep, Instant};
    use tokio_rustls::TlsAcceptor;

    const HUNG: u16 = 1;
    const SLOW_OK: u16 = 2;

    fn addr(port: u16) -> SocketAddr {
        SocketAddr::from(([192, 0, 2, 1], port))
    }

    async fn scripted(addr: SocketAddr) -> Result<u16, &'static str> {
        match addr.port() {
            HUNG => {
                sleep(Duration::from_secs(3600)).await;
                Err("hung")
            }
            SLOW_OK => {
                sleep(Duration::from_millis(10)).await;
                Ok(addr.port())
            }
            _ => Err("refused"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn silent_address_gets_company_after_the_delay() {
        let start = Instant::now();
        let outcome = race(&[addr(HUNG), addr(SLOW_OK)], ATTEMPT_DELAY, scripted).await;
        assert_eq!(outcome.won, Some((addr(SLOW_OK), SLOW_OK)));
        assert!(outcome.failures.is_empty());
        assert_eq!(start.elapsed(), ATTEMPT_DELAY + Duration::from_millis(10));
    }

    #[tokio::test(start_paused = true)]
    async fn failed_attempt_starts_the_next_at_once() {
        let start = Instant::now();
        let outcome = race(&[addr(7), addr(SLOW_OK)], ATTEMPT_DELAY, scripted).await;
        assert_eq!(outcome.won, Some((addr(SLOW_OK), SLOW_OK)));
        assert_eq!(outcome.failures, vec![(addr(7), "refused")]);
        assert_eq!(start.elapsed(), Duration::from_millis(10));
    }

    #[tokio::test(start_paused = true)]
    async fn every_failure_is_reported_in_order() {
        let outcome = race(&[addr(7), addr(8), addr(9)], ATTEMPT_DELAY, scripted).await;
        assert_eq!(outcome.won, None);
        assert_eq!(
            outcome.failures,
            vec![
                (addr(7), "refused"),
                (addr(8), "refused"),
                (addr(9), "refused")
            ]
        );

        let empty = race(&[], ATTEMPT_DELAY, scripted).await;
        assert_eq!(empty.won, None);
        assert!(empty.failures.is_empty());
    }

    fn self_signed(name: &str) -> (CertificateDer<'static>, TlsAcceptor) {
        let certified = rcgen::generate_simple_self_signed(vec![name.to_string()]).unwrap();
        let cert = certified.cert.der().clone();
        let key = PrivatePkcs8KeyDer::from(certified.key_pair.serialize_der());
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![cert.clone()], PrivateKeyDer::Pkcs8(key))
        .unwrap();
        (cert, TlsAcceptor::from(Arc::new(config)))
    }

    /// Accepts TCP, then never says a word: a middlebox swallowing the hello.
    async fn silent_server() -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((tcp, _)) = listener.accept().await {
                held.push(tcp);
            }
        });
        addr
    }

    async fn tls_server(acceptor: TlsAcceptor) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((tcp, _)) = listener.accept().await {
                let acceptor = acceptor.clone();
                tokio::spawn(async move {
                    if let Ok(tls) = acceptor.accept(tcp).await {
                        sleep(Duration::from_secs(30)).await;
                        drop(tls);
                    }
                });
            }
        });
        addr
    }

    #[tokio::test]
    async fn tls_race_skips_silent_and_impostor_addresses() {
        let (cert, acceptor) = self_signed("localhost");
        let (_, impostor_acceptor) = self_signed("impostor.example");
        let silent = silent_server().await;
        let impostor = tls_server(impostor_acceptor).await;
        let live = tls_server(acceptor).await;

        let mut roots = rustls::RootCertStore::empty();
        roots.add(cert).unwrap();
        let config = rustls::ClientConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_root_certificates(roots)
        .with_no_client_auth();
        let connector = TlsConnector::from(Arc::new(config));
        let name = ServerName::try_from("localhost").unwrap();
        let attempt = |addr| {
            let connector = connector.clone();
            let name = name.clone();
            async move {
                let tcp = tcp_attempt(addr).await?;
                connector.connect(name, tcp).await.map_err(DialError::Tls)
            }
        };

        let outcome = timeout(
            Duration::from_secs(5),
            race(&[silent, impostor, live], ATTEMPT_DELAY, attempt),
        )
        .await
        .expect("the live server wins well before the timeout");
        assert_eq!(outcome.won.map(|(addr, _)| addr), Some(live));
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(outcome.failures[0].0, impostor);
        assert_eq!(impostors(&outcome.failures), vec![impostor.ip()]);
    }

    #[test]
    fn summary_prefers_the_tls_error() {
        let failures = vec![
            (
                addr(1),
                DialError::Io(io::Error::from(io::ErrorKind::ConnectionRefused)),
            ),
            (
                addr(2),
                DialError::Tls(io::Error::new(io::ErrorKind::InvalidData, "bad cert")),
            ),
        ];
        match summarize("api.example", 443, failures) {
            DialError::Tls(e) => assert_eq!(e.to_string(), "bad cert"),
            other => panic!("expected the TLS error, got {other:?}"),
        }

        let failures = vec![
            (
                addr(1),
                DialError::Io(io::Error::from(io::ErrorKind::TimedOut)),
            ),
            (
                addr(2),
                DialError::Io(io::Error::from(io::ErrorKind::ConnectionRefused)),
            ),
        ];
        match summarize("api.example", 443, failures) {
            DialError::Io(e) => {
                assert_eq!(e.kind(), io::ErrorKind::ConnectionRefused);
                let text = e.to_string();
                assert!(text.starts_with("connect to api.example:443 failed (192.0.2.1: "));
                assert!(text.contains("; 192.0.2.1: "));
            }
            other => panic!("expected an I/O error, got {other:?}"),
        }
    }
}
