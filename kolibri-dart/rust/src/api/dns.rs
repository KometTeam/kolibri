/// Keep the addresses that worked (socket, media, calls) in `path` across restarts, so a client
/// that connected once still connects while the network's DNS is broken; without a path they stay
/// in memory only. Set once at startup, before the first connect, to a file in the app's private
/// data.
#[flutter_rust_bridge::frb(sync)]
pub fn set_dns_cache_path(path: Option<String>) {
    kolibri_net::set_dns_cache_path(path.map(std::path::PathBuf::from));
}
