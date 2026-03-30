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

#[cfg(test)]
mod tests {
    use super::{
        collect_links, map_api_user_info, map_connection_top_user, pick_best_link,
    };
    use crate::telemt_backend::api_dto::{ApiUserInfo, RuntimeConnectionUserData, UserLinks};
    use crate::telemt_backend::types::TelemtBackendMode;

    #[test]
    fn pick_best_link_prefers_tls_then_secure_then_classic() {
        let tls_first = UserLinks {
            classic: vec!["classic".to_string()],
            secure: vec!["secure".to_string()],
            tls: vec!["tls".to_string()],
        };
        assert_eq!(pick_best_link(&tls_first).as_deref(), Some("tls"));

        let secure_fallback = UserLinks {
            classic: vec!["classic".to_string()],
            secure: vec!["secure".to_string()],
            tls: Vec::new(),
        };
        assert_eq!(pick_best_link(&secure_fallback).as_deref(), Some("secure"));

        let classic_fallback = UserLinks {
            classic: vec!["classic".to_string()],
            secure: Vec::new(),
            tls: Vec::new(),
        };
        assert_eq!(pick_best_link(&classic_fallback).as_deref(), Some("classic"));
        assert_eq!(
            pick_best_link(&UserLinks {
                classic: Vec::new(),
                secure: Vec::new(),
                tls: Vec::new(),
            }),
            None
        );
    }

    #[test]
    fn collect_links_preserves_all_link_groups() {
        let links = UserLinks {
            classic: vec!["classic-1".to_string()],
            secure: vec!["secure-1".to_string()],
            tls: vec!["tls-1".to_string(), "tls-2".to_string()],
        };

        assert_eq!(
            collect_links(&links),
            vec![
                "tls-1".to_string(),
                "tls-2".to_string(),
                "secure-1".to_string(),
                "classic-1".to_string(),
            ]
        );
    }

    #[test]
    fn map_api_user_info_maps_runtime_fields() {
        let user = ApiUserInfo {
            user_ad_tag: Some("promo".to_string()),
            max_tcp_conns: Some(10),
            expiration_rfc3339: Some("2026-04-01T00:00:00Z".to_string()),
            data_quota_bytes: Some(2048),
            max_unique_ips: Some(3),
            current_connections: 2,
            active_unique_ips: 1,
            active_unique_ips_list: vec!["1.1.1.1".to_string()],
            recent_unique_ips: 2,
            recent_unique_ips_list: vec!["1.1.1.1".to_string(), "2.2.2.2".to_string()],
            total_octets: 4096,
            links: UserLinks {
                classic: vec!["classic".to_string()],
                secure: vec!["secure".to_string()],
                tls: vec!["tls".to_string()],
            },
        };

        let mapped = map_api_user_info(TelemtBackendMode::ControlApi, user);

        assert_eq!(mapped.source, TelemtBackendMode::ControlApi);
        assert_eq!(mapped.user_ad_tag.as_deref(), Some("promo"));
        assert_eq!(mapped.max_tcp_conns, Some(10));
        assert_eq!(mapped.data_quota_bytes, Some(2048));
        assert_eq!(mapped.max_unique_ips, Some(3));
        assert_eq!(mapped.current_connections, Some(2));
        assert_eq!(mapped.active_unique_ips, Some(1));
        assert_eq!(mapped.recent_unique_ips, Some(2));
        assert_eq!(mapped.total_octets, Some(4096));
        assert_eq!(mapped.links.len(), 3);
        assert_eq!(mapped.links[0], "tls");
    }

    #[test]
    fn map_connection_top_user_preserves_counters() {
        let mapped = map_connection_top_user(RuntimeConnectionUserData {
            username: "tg_1".to_string(),
            current_connections: 7,
            total_octets: 99,
        });

        assert_eq!(mapped.username, "tg_1");
        assert_eq!(mapped.current_connections, 7);
        assert_eq!(mapped.total_octets, 99);
    }
}
