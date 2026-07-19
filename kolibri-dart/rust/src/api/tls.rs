/// Trust the bundled Минцифры CA (socket, media, calls); off by default, set at startup.
#[flutter_rust_bridge::frb(sync)]
pub fn set_trust_mincifry_ca(enabled: bool) {
    kolibri_net::set_trust_mincifry_ca(enabled);
}

#[flutter_rust_bridge::frb(sync)]
pub fn trust_mincifry_ca() -> bool {
    kolibri_net::trust_mincifry_ca()
}
