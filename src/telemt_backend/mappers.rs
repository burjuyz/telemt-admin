use super::api_dto::{ApiUserInfo, RuntimeConnectionUserData, UserLinks};
use super::types::{
    TelemtBackendMode, TelemtConnectionTopUser, TelemtUserInfo,
};

pub(super) fn pick_best_link(links: &UserLinks) -> Option<String> {
    links
        .tls
        .first()
        .cloned()
        .or_else(|| links.secure.first().cloned())
        .or_else(|| links.classic.first().cloned())
}

fn collect_links(links: &UserLinks) -> Vec<String> {
    links
        .tls
        .iter()
        .chain(links.secure.iter())
        .chain(links.classic.iter())
        .cloned()
        .collect()
}

pub(super) fn map_api_user_info(source: TelemtBackendMode, user: ApiUserInfo) -> TelemtUserInfo {
    TelemtUserInfo {
        source,
        user_ad_tag: user.user_ad_tag,
        max_tcp_conns: user.max_tcp_conns,
        expiration_rfc3339: user.expiration_rfc3339,
        data_quota_bytes: user.data_quota_bytes,
        max_unique_ips: user.max_unique_ips,
        current_connections: Some(user.current_connections),
        active_unique_ips: Some(user.active_unique_ips),
        active_unique_ips_list: user.active_unique_ips_list,
        recent_unique_ips: Some(user.recent_unique_ips),
        recent_unique_ips_list: user.recent_unique_ips_list,
        total_octets: Some(user.total_octets),
        links: collect_links(&user.links),
    }
}

pub(super) fn map_connection_top_user(user: RuntimeConnectionUserData) -> TelemtConnectionTopUser {
    TelemtConnectionTopUser {
        username: user.username,
        current_connections: user.current_connections,
        total_octets: user.total_octets,
    }
}
