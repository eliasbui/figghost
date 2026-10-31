//! Network control for figghost: which app uses which connection of the Mac, how fast, and before which other app.
//!
//! Without an administrator's extension of the system, the way in is the system's own proxy settings: Figghost sets
//! the SOCKS proxy and the secure web proxy of each network service to a proxy of its own on 127.0.0.1, so every app
//! that follows those settings (browsers, most apps built on the system's network code) connects through it. The
//! proxy finds which app connected by the port it came from, and takes the wire that app has on the diagram: the
//! system's way, one link pinned by its interface, or every link in turn (net-merge, from `core-netmerge`). Apps that
//! ignore the system's proxy are only watched, through `nettop`.

pub mod bucket;
pub mod handshake;
pub mod monitor;
pub mod owner;
pub mod proxy;
pub mod rules;
pub mod sysproxy;
pub mod tether;
