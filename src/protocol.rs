//! Versioned wire protocol helpers and HTTP header constants.

/// Current wire protocol major version.
pub const PROTOCOL_VERSION: u32 = 1;

/// Request header name for protocol version.
pub const HEADER_PROTOCOL: &str = "darial-protocol";

/// Header for capability advertisement.
pub const HEADER_CAPABILITIES: &str = "darial-capabilities";

/// Content type for private envelopes (JSON wrapper).
pub const CONTENT_TYPE_ENVELOPE: &str = "application/darial-envelope+json";

/// Content type for direct IR JSON.
pub const CONTENT_TYPE_IR: &str = "application/json";

/// Parse and validate protocol version header.
pub fn parse_protocol_version(header: Option<&str>) -> Result<u32, String> {
    let Some(raw) = header else {
        return Err("missing Darial-Protocol header".into());
    };
    let v: u32 = raw
        .trim()
        .parse()
        .map_err(|_| "invalid Darial-Protocol header".to_string())?;
    if v != PROTOCOL_VERSION {
        return Err(format!("unsupported protocol version {v}"));
    }
    Ok(v)
}

/// Envelope direction for AAD binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Client → gateway request.
    Request,
    /// Gateway → client response.
    Response,
}

impl Direction {
    /// Stable label for AAD.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Request => "request",
            Self::Response => "response",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_and_wrong_version() {
        assert!(parse_protocol_version(None).is_err());
        assert!(parse_protocol_version(Some("99")).is_err());
        assert_eq!(parse_protocol_version(Some("1")).unwrap(), 1);
    }
}
