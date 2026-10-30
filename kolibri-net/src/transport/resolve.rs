//! Where a connect gets its addresses from: the system resolver, then the
//! addresses that last worked for the host, then DNS-over-HTTPS (see
//! [`super::doh`]). A blocking DNS refuses a name by pointing it nowhere
//! (`0.0.0.0`, `::`, loopback); such answers count as no answer at all, so
//! they never replace addresses that are known to work.
//!
//! The cache lives in memory and, once [`set_dns_cache_path`] names a file,
//! survives restarts: a client that connected once keeps connecting while the
//! network's DNS is broken, filtered or hijacked.

use std::io;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use tokio::time::timeout;

/// System resolver budget when there is nothing cached to fall back on.
pub(crate) const LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);
/// System resolver budget when cached addresses can stand in for it.
pub(crate) const LOOKUP_TIMEOUT_CACHED: Duration = Duration::from_millis(1500);

const MAX_ADDRS_PER_HOST: usize = 8;
const MAX_HOSTS: usize = 32;

/// `host` written as an IP address (`[...]` around IPv6 allowed).
pub(crate) fn ip_literal(host: &str) -> Option<IpAddr> {
    let bare = host
        .strip_prefix('[')
        .and_then(|h| h.strip_suffix(']'))
        .unwrap_or(host);
    bare.parse().ok()
}

pub(crate) fn is_localhost(host: &str) -> bool {
    let host = normalize(host);
    host == "localhost" || host.ends_with(".localhost")
}

/// An answer that leads nowhere: how ad-blocking and censoring resolvers
/// refuse a name. Loopback is legitimate only for `localhost` itself.
pub(crate) fn is_sinkhole(ip: IpAddr, host: &str) -> bool {
    let ip = ip.to_canonical();
    ip.is_unspecified() || (ip.is_loopback() && !is_localhost(host))
}

/// System resolver answers for `host`, sinkholes dropped. `Ok` with an empty
/// list means the resolver answered, but with nothing usable.
pub(crate) async fn system_lookup(
    host: &str,
    port: u16,
    limit: Duration,
) -> io::Result<Vec<IpAddr>> {
    let found = timeout(limit, tokio::net::lookup_host((host, port)))
        .await
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::TimedOut,
                format!("resolving {host} timed out"),
            )
        })??;
    let mut out = Vec::new();
    for addr in found {
        let ip = addr.ip();
        if !is_sinkhole(ip, host) && !out.contains(&ip) {
            out.push(ip);
        }
    }
    Ok(out)
}

/// The order addresses are tried in: the last winner if the resolver still
/// lists it, then the resolver's answers with address families alternating
/// (RFC 8305), then cached addresses the resolver no longer lists.
pub(crate) fn order(fresh: &[IpAddr], cached: &[IpAddr]) -> Vec<IpAddr> {
    let mut out = Vec::with_capacity(fresh.len() + cached.len());
    if let Some(winner) = cached.first().filter(|ip| fresh.contains(ip)) {
        out.push(*winner);
    }
    for ip in interleave(fresh).into_iter().chain(cached.iter().copied()) {
        if !out.contains(&ip) {
            out.push(ip);
        }
    }
    out
}

fn interleave(addrs: &[IpAddr]) -> Vec<IpAddr> {
    let Some(first) = addrs.first() else {
        return Vec::new();
    };
    let mut lead = addrs.iter().filter(|a| a.is_ipv6() == first.is_ipv6());
    let mut other = addrs.iter().filter(|a| a.is_ipv6() != first.is_ipv6());
    let mut out = Vec::with_capacity(addrs.len());
    loop {
        match (lead.next(), other.next()) {
            (None, None) => return out,
            (a, b) => out.extend(a.into_iter().chain(b).copied()),
        }
    }
}

fn normalize(host: &str) -> String {
    host.trim_end_matches('.').to_ascii_lowercase()
}

/// Addresses that worked, per host, best first; the most recently connected
/// host comes first and the oldest is evicted past [`MAX_HOSTS`].
#[derive(Debug, Default, PartialEq)]
pub(crate) struct DnsCache {
    hosts: Vec<(String, Vec<IpAddr>)>,
}

impl DnsCache {
    pub(crate) const fn new() -> Self {
        Self { hosts: Vec::new() }
    }

    pub(crate) fn get(&self, host: &str) -> Vec<IpAddr> {
        let host = normalize(host);
        self.hosts
            .iter()
            .find(|(h, _)| *h == host)
            .map(|(_, addrs)| addrs.clone())
            .unwrap_or_default()
    }

    /// Record a successful connect to `winner`: it goes first, then the rest of
    /// what the resolver returned (`seen`) and of what was cached. `rejected`
    /// addresses presented a certificate that is not the host's while `winner`
    /// presented a valid one, so they are not the host and are dropped. Returns
    /// whether the stored addresses changed.
    pub(crate) fn remember(
        &mut self,
        host: &str,
        winner: IpAddr,
        seen: &[IpAddr],
        rejected: &[IpAddr],
    ) -> bool {
        let host = normalize(host);
        let old = match self.hosts.iter().position(|(h, _)| *h == host) {
            Some(i) => self.hosts.remove(i).1,
            None => Vec::new(),
        };
        let mut addrs = vec![winner];
        for ip in seen.iter().chain(&old).copied() {
            if addrs.len() == MAX_ADDRS_PER_HOST {
                break;
            }
            if !addrs.contains(&ip) && !rejected.contains(&ip) && !is_sinkhole(ip, &host) {
                addrs.push(ip);
            }
        }
        let changed = addrs != old;
        self.hosts.insert(0, (host, addrs));
        self.hosts.truncate(MAX_HOSTS);
        changed
    }

    /// One host per line: the name, then its addresses, space separated.
    /// Malformed lines and sinkhole addresses are skipped.
    pub(crate) fn parse(text: &str) -> Self {
        let mut cache = Self::new();
        for line in text.lines() {
            let mut fields = line.split_whitespace();
            let Some(host) = fields.next().map(normalize) else {
                continue;
            };
            if cache.hosts.iter().any(|(h, _)| *h == host) {
                continue;
            }
            let mut addrs: Vec<IpAddr> = Vec::new();
            for ip in fields.filter_map(|f| f.parse().ok()) {
                if !addrs.contains(&ip) && !is_sinkhole(ip, &host) {
                    addrs.push(ip);
                }
            }
            addrs.truncate(MAX_ADDRS_PER_HOST);
            if !addrs.is_empty() && cache.hosts.len() < MAX_HOSTS {
                cache.hosts.push((host, addrs));
            }
        }
        cache
    }

    pub(crate) fn serialize(&self) -> String {
        let mut out = String::new();
        for (host, addrs) in &self.hosts {
            out.push_str(host);
            for ip in addrs {
                out.push(' ');
                out.push_str(&ip.to_string());
            }
            out.push('\n');
        }
        out
    }

    /// Take the hosts of `other` this cache does not know yet, after its own.
    fn absorb(&mut self, other: DnsCache) {
        for (host, addrs) in other.hosts {
            if self.hosts.len() == MAX_HOSTS {
                break;
            }
            if !self.hosts.iter().any(|(h, _)| *h == host) {
                self.hosts.push((host, addrs));
            }
        }
    }
}

struct Store {
    cache: DnsCache,
    path: Option<PathBuf>,
}

static STORE: Mutex<Store> = Mutex::new(Store {
    cache: DnsCache::new(),
    path: None,
});

/// Serializes file writes, so the file always ends up with the latest state.
static WRITE: Mutex<()> = Mutex::new(());

fn store() -> MutexGuard<'static, Store> {
    STORE.lock().unwrap_or_else(|e| e.into_inner())
}

/// Persist the addresses that worked to `path` and load what an earlier run
/// stored there; `None` keeps the cache in memory only (the default). Set once
/// at startup, before the first connect, to a file in the app's private data.
pub fn set_dns_cache_path(path: Option<PathBuf>) {
    let loaded = path
        .as_deref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|text| DnsCache::parse(&text));
    let mut store = store();
    store.path = path;
    if let Some(cache) = loaded {
        store.cache.absorb(cache);
    }
}

pub(crate) fn cached(host: &str) -> Vec<IpAddr> {
    store().cache.get(host)
}

/// [`DnsCache::remember`] on the process-wide cache, written through to the
/// cache file when the addresses changed.
pub(crate) fn remember(host: &str, winner: IpAddr, seen: &[IpAddr], rejected: &[IpAddr]) {
    let path = {
        let mut store = store();
        if !store.cache.remember(host, winner, seen, rejected) {
            return;
        }
        store.path.clone()
    };
    let Some(path) = path else {
        return;
    };
    let write = move || {
        let _write = WRITE.lock().unwrap_or_else(|e| e.into_inner());
        let text = store().cache.serialize();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let mut tmp = path.clone().into_os_string();
        tmp.push(".tmp");
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    };
    match tokio::runtime::Handle::try_current() {
        Ok(rt) => drop(rt.spawn_blocking(write)),
        Err(_) => write(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    fn v4(last: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(192, 0, 2, last))
    }

    fn v6(last: u16) -> IpAddr {
        IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, last))
    }

    #[test]
    fn literals_skip_resolution() {
        assert_eq!(ip_literal("192.0.2.1"), Some(v4(1)));
        assert_eq!(ip_literal("[2001:db8::1]"), Some(v6(1)));
        assert_eq!(ip_literal("2001:db8::1"), Some(v6(1)));
        assert_eq!(ip_literal("api.example"), None);
    }

    #[test]
    fn sinkholes_are_not_answers() {
        let host = "api.example";
        assert!(is_sinkhole(IpAddr::V4(Ipv4Addr::UNSPECIFIED), host));
        assert!(is_sinkhole(IpAddr::V6(Ipv6Addr::UNSPECIFIED), host));
        assert!(is_sinkhole(IpAddr::V4(Ipv4Addr::LOCALHOST), host));
        assert!(is_sinkhole(IpAddr::V6(Ipv6Addr::LOCALHOST), host));
        assert!(is_sinkhole("::ffff:0.0.0.0".parse().unwrap(), host));
        assert!(!is_sinkhole(v4(1), host));
        assert!(!is_sinkhole("10.0.0.1".parse().unwrap(), host));
        assert!(!is_sinkhole(IpAddr::V4(Ipv4Addr::LOCALHOST), "localhost"));
        assert!(!is_sinkhole(IpAddr::V4(Ipv4Addr::LOCALHOST), "LocalHost."));
    }

    #[test]
    fn families_alternate_from_the_first_answer() {
        assert_eq!(
            interleave(&[v6(1), v6(2), v6(3), v4(1), v4(2)]),
            vec![v6(1), v4(1), v6(2), v4(2), v6(3)]
        );
        assert_eq!(interleave(&[v4(1), v4(2)]), vec![v4(1), v4(2)]);
        assert!(interleave(&[]).is_empty());
    }

    #[test]
    fn last_winner_leads_and_stale_cache_trails() {
        assert_eq!(
            order(&[v4(1), v4(2), v4(3)], &[v4(3), v4(9)]),
            vec![v4(3), v4(1), v4(2), v4(9)]
        );
        assert_eq!(
            order(&[v4(1), v4(2)], &[v4(9), v4(2)]),
            vec![v4(1), v4(2), v4(9)]
        );
        assert_eq!(order(&[], &[v4(9), v4(8)]), vec![v4(9), v4(8)]);
        assert!(order(&[], &[]).is_empty());
    }

    #[test]
    fn remember_puts_winner_first_and_drops_rejected() {
        let mut cache = DnsCache::new();
        assert!(cache.remember("API.example.", v4(2), &[v4(1), v4(2), v4(3)], &[]));
        assert_eq!(cache.get("api.example"), vec![v4(2), v4(1), v4(3)]);

        assert!(!cache.remember("api.example", v4(2), &[v4(1), v4(2), v4(3)], &[]));

        assert!(cache.remember("api.example", v4(3), &[v4(7)], &[v4(7), v4(1)]));
        assert_eq!(cache.get("api.example"), vec![v4(3), v4(2)]);
    }

    #[test]
    fn remember_caps_addresses_and_hosts() {
        let mut cache = DnsCache::new();
        let many: Vec<IpAddr> = (1..=20).map(v4).collect();
        cache.remember("api.example", v4(1), &many, &[]);
        assert_eq!(cache.get("api.example").len(), MAX_ADDRS_PER_HOST);

        for i in 0..MAX_HOSTS + 5 {
            cache.remember(&format!("h{i}.example"), v4(1), &[], &[]);
        }
        assert_eq!(cache.hosts.len(), MAX_HOSTS);
        assert!(cache.get("api.example").is_empty(), "oldest host evicted");
        assert_eq!(
            cache.get(&format!("h{}.example", MAX_HOSTS + 4)),
            vec![v4(1)]
        );
    }

    #[test]
    fn file_format_roundtrips_and_skips_garbage() {
        let mut cache = DnsCache::new();
        cache.remember("a.example", v4(1), &[v6(1)], &[]);
        cache.remember("b.example", v4(2), &[], &[]);
        let text = cache.serialize();
        assert_eq!(
            text,
            "b.example 192.0.2.2\na.example 192.0.2.1 2001:db8::1\n"
        );
        assert_eq!(DnsCache::parse(&text), cache);

        let parsed = DnsCache::parse(
            "\n  \nc.example nonsense 0.0.0.0 192.0.2.3\nd.example\nc.example 192.0.2.4\n",
        );
        assert_eq!(parsed.get("c.example"), vec![v4(3)]);
        assert!(parsed.get("d.example").is_empty());
        assert_eq!(parsed.hosts.len(), 1);
    }

    #[test]
    fn absorb_keeps_what_memory_already_knows() {
        let mut memory = DnsCache::new();
        memory.remember("a.example", v4(1), &[], &[]);
        memory.absorb(DnsCache::parse(
            "a.example 192.0.2.9\nb.example 192.0.2.2\n",
        ));
        assert_eq!(memory.get("a.example"), vec![v4(1)]);
        assert_eq!(memory.get("b.example"), vec![v4(2)]);
    }
}
