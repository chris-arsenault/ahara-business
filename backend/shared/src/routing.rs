use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoutingPolicy {
    Allowlist,
    Catchall,
}

impl RoutingPolicy {
    pub fn as_db_value(self) -> &'static str {
        match self {
            Self::Allowlist => "allowlist",
            Self::Catchall => "catchall",
        }
    }
}

impl FromStr for RoutingPolicy {
    type Err = RoutingParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "allowlist" => Ok(Self::Allowlist),
            "catchall" => Ok(Self::Catchall),
            _ => Err(RoutingParseError::InvalidPolicy(value.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRoute {
    pub address: String,
    pub base_local_part: String,
    pub plus_tag: Option<String>,
    pub domain: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RoutingParseError {
    #[error("invalid routing policy: {0}")]
    InvalidPolicy(String),

    #[error("address must contain exactly one @")]
    InvalidAddressShape,

    #[error("address local part is empty")]
    EmptyLocalPart,

    #[error("address domain is empty")]
    EmptyDomain,
}

pub fn parse_route(address: &str) -> Result<ParsedRoute, RoutingParseError> {
    let address = address.trim();
    let (local, domain) = address
        .split_once('@')
        .filter(|(_, domain)| !domain.contains('@'))
        .ok_or(RoutingParseError::InvalidAddressShape)?;

    if local.is_empty() {
        return Err(RoutingParseError::EmptyLocalPart);
    }
    if domain.is_empty() {
        return Err(RoutingParseError::EmptyDomain);
    }

    let (base_local, plus_tag) = match local.split_once('+') {
        Some(("", _tag)) => {
            return Err(RoutingParseError::EmptyLocalPart);
        }
        Some((base, tag)) => (base, Some(tag.to_string())),
        None => (local, None),
    };

    Ok(ParsedRoute {
        address: address.to_string(),
        base_local_part: base_local.to_ascii_lowercase(),
        plus_tag,
        domain: domain.to_ascii_lowercase(),
    })
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::{RoutingParseError, RoutingPolicy, parse_route};

    #[test]
    fn parses_known_routing_policies() {
        assert_eq!(
            RoutingPolicy::from_str("allowlist").unwrap(),
            RoutingPolicy::Allowlist
        );
        assert_eq!(
            RoutingPolicy::from_str("CATCHALL").unwrap(),
            RoutingPolicy::Catchall
        );
        assert_eq!(RoutingPolicy::Allowlist.as_db_value(), "allowlist");
    }

    #[test]
    fn rejects_invalid_routing_policy() {
        assert!(matches!(
            RoutingPolicy::from_str("forward-all"),
            Err(RoutingParseError::InvalidPolicy(_))
        ));
    }

    #[test]
    fn parse_route_normalizes_base_local_part_and_domain() {
        let route = parse_route("Contact@Ahara.IO").unwrap();

        assert_eq!(route.base_local_part, "contact");
        assert_eq!(route.domain, "ahara.io");
        assert_eq!(route.plus_tag, None);
        assert_eq!(route.address, "Contact@Ahara.IO");
    }

    #[test]
    fn parse_route_preserves_plus_tag_suffix() {
        let route = parse_route("Contact+Sales-Q2@Ahara.IO").unwrap();

        assert_eq!(route.base_local_part, "contact");
        assert_eq!(route.plus_tag.as_deref(), Some("Sales-Q2"));
        assert_eq!(route.domain, "ahara.io");
    }

    #[test]
    fn parse_route_rejects_invalid_addresses() {
        assert_eq!(
            parse_route("missing-at"),
            Err(RoutingParseError::InvalidAddressShape)
        );
        assert_eq!(
            parse_route("@ahara.io"),
            Err(RoutingParseError::EmptyLocalPart)
        );
        assert_eq!(parse_route("contact@"), Err(RoutingParseError::EmptyDomain));
        assert_eq!(
            parse_route("contact@ahara.io@example.test"),
            Err(RoutingParseError::InvalidAddressShape)
        );
    }
}
