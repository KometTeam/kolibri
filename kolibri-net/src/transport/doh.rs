//! DNS-over-HTTPS (RFC 8484), the last resort when the system resolver fails,
//! answers with sinkholes, or hands out addresses that do not work, and nothing
//! is cached. Resolvers are dialed by IP and their certificates are issued for
//! those IPs, so the fallback itself needs no DNS. Asks for A records only.

use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use rustls::pki_types::ServerName;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;
use tokio::time::timeout;

use super::dial::{race, ATTEMPT_DELAY};
use super::resolve::is_sinkhole;
use super::tls::build_connector;
use crate::media::http::read_response;

/// Tried in this order, the next one joining every [`ATTEMPT_DELAY`]: Yandex
/// first, as it stays reachable on Russian networks, then Google, Cloudflare.
const RESOLVERS: [Ipv4Addr; 3] = [
    Ipv4Addr::new(77, 88, 8, 8),
    Ipv4Addr::new(8, 8, 8, 8),
    Ipv4Addr::new(1, 1, 1, 1),
];

const TIMEOUT: Duration = Duration::from_secs(4);

const TYPE_A: u16 = 1;
const CLASS_IN: u16 = 1;

/// Usable IPv4 addresses of `host`, empty when no resolver answered in time.
pub(crate) async fn lookup(host: &str) -> Vec<IpAddr> {
    let Some(query) = encode_query(host) else {
        return Vec::new();
    };
    let query = Arc::new(query);
    let host: Arc<str> = Arc::from(host);
    let resolvers: Vec<SocketAddr> = RESOLVERS
        .iter()
        .map(|ip| SocketAddr::new(IpAddr::V4(*ip), 443))
        .collect();
    let attempt = |resolver| {
        let query = query.clone();
        let host = host.clone();
        async move { ask(resolver, &query, &host).await }
    };
    match timeout(TIMEOUT, race(&resolvers, ATTEMPT_DELAY, attempt)).await {
        Ok(outcome) => outcome.won.map(|(_, addrs)| addrs).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

async fn ask(resolver: SocketAddr, query: &[u8], host: &str) -> io::Result<Vec<IpAddr>> {
    let tcp = TcpStream::connect(resolver).await?;
    tcp.set_nodelay(true).ok();
    let connector = build_connector(false).map_err(io::Error::other)?;
    let mut tls = connector
        .connect(ServerName::from(resolver.ip()), tcp)
        .await?;

    let head = format!(
        "POST /dns-query HTTP/1.1\r\nHost: {}\r\nAccept: application/dns-message\r\n\
         Content-Type: application/dns-message\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        resolver.ip(),
        query.len()
    );
    tls.write_all(head.as_bytes()).await?;
    tls.write_all(query).await?;
    tls.flush().await?;

    let response = read_response(&mut tls, TIMEOUT)
        .await
        .map_err(io::Error::other)?;
    if response.status != 200 {
        return Err(io::Error::other(format!(
            "DoH {resolver}: HTTP {}",
            response.status
        )));
    }
    let addrs: Vec<IpAddr> = parse_response(&response.body)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "malformed DNS answer"))?
        .into_iter()
        .map(IpAddr::V4)
        .filter(|ip| !is_sinkhole(*ip, host))
        .collect();
    if addrs.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("DoH {resolver}: no address for {host}"),
        ));
    }
    Ok(addrs)
}

/// A recursive A/IN query. ID 0, as RFC 8484 recommends for DoH.
fn encode_query(host: &str) -> Option<Vec<u8>> {
    let name = host.trim_end_matches('.');
    if name.is_empty() || name.len() > 253 {
        return None;
    }
    let mut msg = vec![0, 0, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, 0];
    for label in name.split('.') {
        if label.is_empty() || label.len() > 63 {
            return None;
        }
        msg.push(label.len() as u8);
        msg.extend_from_slice(label.as_bytes());
    }
    msg.push(0);
    msg.extend_from_slice(&TYPE_A.to_be_bytes());
    msg.extend_from_slice(&CLASS_IN.to_be_bytes());
    Some(msg)
}

/// A records of a DNS response, whatever name they are for: CNAME chains come
/// back with the final A records in the same answer. An error RCODE (e.g.
/// NXDOMAIN) is an empty answer; a malformed message is `None`.
fn parse_response(msg: &[u8]) -> Option<Vec<Ipv4Addr>> {
    let header = msg.get(..12)?;
    let flags = u16::from_be_bytes([header[2], header[3]]);
    if flags & 0x8000 == 0 {
        return None;
    }
    if flags & 0x000f != 0 {
        return Some(Vec::new());
    }
    let questions = u16::from_be_bytes([header[4], header[5]]);
    let answers = u16::from_be_bytes([header[6], header[7]]);

    let mut pos = 12;
    for _ in 0..questions {
        pos = skip_name(msg, pos)? + 4;
    }
    let mut out = Vec::new();
    for _ in 0..answers {
        pos = skip_name(msg, pos)?;
        let fixed = msg.get(pos..pos + 10)?;
        let rtype = u16::from_be_bytes([fixed[0], fixed[1]]);
        let class = u16::from_be_bytes([fixed[2], fixed[3]]);
        let len = u16::from_be_bytes([fixed[8], fixed[9]]) as usize;
        pos += 10;
        let data = msg.get(pos..pos + len)?;
        if rtype == TYPE_A && class == CLASS_IN && len == 4 {
            out.push(Ipv4Addr::new(data[0], data[1], data[2], data[3]));
        }
        pos += len;
    }
    Some(out)
}

/// Offset just past the name at `pos`; a compression pointer ends the name.
fn skip_name(msg: &[u8], mut pos: usize) -> Option<usize> {
    loop {
        let len = *msg.get(pos)?;
        match len & 0xc0 {
            0xc0 => {
                msg.get(pos + 1)?;
                return Some(pos + 2);
            }
            0x00 if len == 0 => return Some(pos + 1),
            0x00 => pos += 1 + len as usize,
            _ => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const QUERY: &[u8] = b"\x00\x00\x01\x00\x00\x01\x00\x00\x00\x00\x00\x00\
        \x03api\x07example\x00\x00\x01\x00\x01";

    fn response(rcode: u8, answers: &[&[u8]]) -> Vec<u8> {
        let mut msg = QUERY.to_vec();
        msg[2] = 0x81;
        msg[3] = 0x80 | rcode;
        msg[7] = answers.len() as u8;
        for answer in answers {
            msg.extend_from_slice(answer);
        }
        msg
    }

    #[test]
    fn query_matches_wire_format() {
        assert_eq!(encode_query("api.example").as_deref(), Some(QUERY));
        assert_eq!(encode_query("api.example.").as_deref(), Some(QUERY));
        assert!(encode_query("").is_none());
        assert!(encode_query("a..example").is_none());
        assert!(encode_query(&format!("{}.example", "x".repeat(64))).is_none());
    }

    #[test]
    fn reads_a_records_through_a_cname() {
        let cname: &[u8] = b"\xc0\x0c\x00\x05\x00\x01\x00\x00\x00\x3c\x00\x06\
            \x03cdn\xc0\x10";
        let a1: &[u8] = b"\x03cdn\xc0\x10\x00\x01\x00\x01\x00\x00\x00\x3c\x00\x04\xc0\x00\x02\x01";
        let a2: &[u8] = b"\xc0\x0c\x00\x01\x00\x01\x00\x00\x00\x3c\x00\x04\xc6\x33\x64\x07";
        let aaaa: &[u8] = b"\xc0\x0c\x00\x1c\x00\x01\x00\x00\x00\x3c\x00\x10\
            \x20\x01\x0d\xb8\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x01";
        assert_eq!(
            parse_response(&response(0, &[cname, a1, aaaa, a2])),
            Some(vec![
                Ipv4Addr::new(192, 0, 2, 1),
                Ipv4Addr::new(198, 51, 100, 7)
            ])
        );
    }

    #[test]
    fn error_rcode_is_an_empty_answer() {
        assert_eq!(parse_response(&response(3, &[])), Some(Vec::new()));
    }

    #[test]
    fn malformed_messages_are_rejected() {
        assert_eq!(parse_response(QUERY), None, "not a response");
        assert_eq!(parse_response(&response(0, &[])[..8]), None);
        let truncated: &[u8] = b"\xc0\x0c\x00\x01\x00\x01\x00\x00\x00\x3c\x00\x04\xc0\x00";
        assert_eq!(parse_response(&response(0, &[truncated])), None);
        let bad_label: &[u8] = b"\x80";
        assert_eq!(parse_response(&response(0, &[bad_label])), None);
    }
}
