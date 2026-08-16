//! The parts of Moonlight that are not the user interface: the mihomo
//! supervisor, its RESTful API client, the config builder, the subscription
//! client, and the Windows system-proxy layer.

pub mod api;
pub mod country;
pub mod format;
pub mod helper;
pub mod mihomo_config;
pub mod models;
pub mod preferences;
pub mod process;
pub mod share_link;
pub mod split_rule;
pub mod subscription;
pub mod system_proxy;

pub use models::{
    AppEntry, AppLocale, ConnectionState, Node, SplitMode, SubscriptionInfo, TunnelMode,
};
