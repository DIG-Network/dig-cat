//! CAT metadata — human-facing name/ticker/decimals for an asset id.
//!
//! On-chain a CAT is just its asset id; the display metadata (name, ticker, decimals, logo) lives in
//! off-chain registries. [`CatMetadata`] is the pure data type — always available — while
//! [`resolve_metadata`] (behind the default-OFF `dexie` feature) fetches it from the dexie registry.
//! Metadata is display-only and never affects a spend, so this is the crate's ONLY network call
//! (INV-1) and it is opt-in.

use chia_protocol::Bytes32;

/// Human-facing metadata for a CAT. All descriptive fields are optional (a registry may not carry
/// them); `decimals` always has a value (defaulting to 3, the Chia CAT convention, when unknown).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatMetadata {
    /// The asset id this metadata describes.
    pub asset_id: Bytes32,
    /// The full token name (e.g. "Spacebucks").
    pub name: Option<String>,
    /// The ticker/code (e.g. "SBX").
    pub code: Option<String>,
    /// Display decimals (base units per whole token = 10^decimals). Defaults to 3.
    pub decimals: u8,
    /// A logo/icon URL, if the registry provides one.
    pub logo_url: Option<String>,
    /// A human description.
    pub description: Option<String>,
}

/// The Chia CAT convention when a registry does not state a token's decimals.
#[cfg(feature = "dexie")]
const DEFAULT_DECIMALS: u8 = 3;

/// Resolve display metadata for `asset_id` from the dexie registry (feature `dexie`).
///
/// Performs ONE HTTPS GET to a fixed host + path with the 64-char hex asset id as the only dynamic
/// segment (SSRF-safe: no user free-text, no cross-host redirects, short timeout). Returns
/// `Ok(None)` when the asset is unknown (any non-success response); returns [`crate::CatError::Dexie`]
/// only on a transport or JSON-parse failure.
#[cfg(feature = "dexie")]
pub fn resolve_metadata(asset_id: Bytes32) -> Result<Option<CatMetadata>, crate::CatError> {
    use crate::CatError;
    use std::time::Duration;

    let url = format!("https://api.dexie.space/v1/assets/{}", hex_lower(asset_id));
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(10))
        .redirects(0) // never follow a redirect off the fixed host (SSRF hardening)
        .build();

    match agent.get(&url).call() {
        Ok(response) => {
            let body = response
                .into_string()
                .map_err(|e| CatError::Dexie(format!("reading response body: {e}")))?;
            parse_metadata(asset_id, &body)
        }
        // A non-2xx status (e.g. 404 for an unknown asset) is "not found", not an error.
        Err(ureq::Error::Status(_, _)) => Ok(None),
        Err(ureq::Error::Transport(t)) => Err(CatError::Dexie(format!("transport error: {t}"))),
    }
}

/// Lowercase hex of an asset id — the only dynamic part of the request URL.
#[cfg(feature = "dexie")]
fn hex_lower(asset_id: Bytes32) -> String {
    let mut s = String::with_capacity(64);
    for byte in asset_id.to_bytes() {
        s.push_str(&format!("{byte:02x}"));
    }
    s
}

/// Parse a dexie by-id response body into [`CatMetadata`]. Separated from the network call so the
/// mapping is unit-testable against a captured fixture (no network in CI).
#[cfg(feature = "dexie")]
fn parse_metadata(asset_id: Bytes32, body: &str) -> Result<Option<CatMetadata>, crate::CatError> {
    use crate::CatError;

    let response: DexieResponse = serde_json::from_str(body)
        .map_err(|e| CatError::Dexie(format!("parsing dexie JSON: {e}")))?;

    let Some(asset) = response.asset.filter(|_| response.success) else {
        return Ok(None);
    };

    Ok(Some(CatMetadata {
        asset_id,
        name: asset.name,
        code: asset.code,
        decimals: asset.denom.map_or(DEFAULT_DECIMALS, decimals_from_denom),
        logo_url: asset.icon,
        description: asset.description,
    }))
}

/// Convert a dexie `denom` (base units per whole token, a power of ten) to display decimals:
/// `1000 -> 3`, `1 -> 0`.
#[cfg(feature = "dexie")]
fn decimals_from_denom(denom: u64) -> u8 {
    let mut value = denom.max(1);
    let mut decimals = 0u8;
    while value > 1 && value % 10 == 0 {
        value /= 10;
        decimals += 1;
    }
    decimals
}

/// The dexie by-id response envelope: `{ "success": bool, "asset": { ... } }`.
#[cfg(feature = "dexie")]
#[derive(serde::Deserialize)]
struct DexieResponse {
    #[serde(default)]
    success: bool,
    asset: Option<DexieAsset>,
}

/// The dexie asset object. Only the fields dig-cat surfaces are captured; the rest are ignored.
#[cfg(feature = "dexie")]
#[derive(serde::Deserialize)]
struct DexieAsset {
    code: Option<String>,
    name: Option<String>,
    description: Option<String>,
    /// Base units per whole token (a power of ten); `1000` means 3 decimals.
    denom: Option<u64>,
    /// A logo URL where dexie provides one.
    icon: Option<String>,
}

#[cfg(all(test, feature = "dexie"))]
mod tests {
    use super::*;

    const SBX: Bytes32 = Bytes32::new([0xABu8; 32]);

    #[test]
    fn parses_a_real_dexie_asset_fixture() {
        // A captured live dexie asset object (SBX), so the parse path is covered without the network.
        let body = include_str!("../tests/fixtures/dexie_sbx.json");
        let meta = parse_metadata(SBX, body).unwrap().expect("SBX is known");
        assert_eq!(meta.code.as_deref(), Some("SBX"));
        assert_eq!(meta.name.as_deref(), Some("Spacebucks"));
        assert_eq!(meta.decimals, 3, "denom 1000 -> 3 decimals");
        assert!(meta.description.unwrap().contains("galactic"));
        assert_eq!(meta.asset_id, SBX);
    }

    #[test]
    fn unknown_asset_returns_none() {
        let body = r#"{"success":false,"error_message":"Not Found"}"#;
        assert_eq!(parse_metadata(SBX, body).unwrap(), None);
    }

    #[test]
    fn malformed_json_is_an_error() {
        assert!(parse_metadata(SBX, "not json").is_err());
    }

    #[test]
    fn denom_maps_to_decimals() {
        assert_eq!(decimals_from_denom(1000), 3);
        assert_eq!(decimals_from_denom(1), 0);
        assert_eq!(decimals_from_denom(1_000_000), 6);
    }

    #[test]
    fn missing_denom_defaults_to_three() {
        let body = r#"{"success":true,"asset":{"code":"X","name":"X Token"}}"#;
        let meta = parse_metadata(SBX, body).unwrap().unwrap();
        assert_eq!(meta.decimals, DEFAULT_DECIMALS);
    }

    // A live smoke test against dexie's by-id endpoint. Ignored in CI (network); the endpoint's
    // availability is tracked separately — the parse path above is the covered contract.
    #[test]
    #[ignore]
    fn live_resolve_sbx() {
        let sbx = Bytes32::new(hex_to_32(
            "a628c1c2c6fcb74d53746157e438e108eab5c0bb3e5c80ff9b1910b3e4832913",
        ));
        let meta = resolve_metadata(sbx).unwrap();
        println!("live dexie SBX metadata: {meta:?}");
    }

    #[cfg(test)]
    fn hex_to_32(s: &str) -> [u8; 32] {
        let mut out = [0u8; 32];
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap();
        }
        out
    }
}
