//! SAC (Stellar Asset Contract) event extraction.
//!
//! SAC transfer events have the topic structure:
//!   ["transfer", from_address, to_address, asset_symbol]
//! with data being the amount as an i128 ScVal.

use crate::recorder::manifest::{AssetFlow, FlowDirection};
use crate::recorder::xdr::DiagnosticEvent;

/// Known SAC contract topic that signals a transfer.
pub const SAC_TRANSFER_TOPIC: &str = "transfer";

/// XLM decimals
pub const XLM_DECIMALS: u8 = 7;
/// Default SEP-41 decimals when not known
pub const DEFAULT_DECIMALS: u8 = 7;

/// Extract asset flows from diagnostic events for a given invoking account.
pub fn extract_asset_flows(
    events: &[DiagnosticEvent],
    invoking_account: &str,
) -> Vec<AssetFlow> {
    let mut flows = Vec::new();

    for event in events {
        // Only process contract events
        if event.event_type != "contract" {
            continue;
        }

        let contract_id = match &event.contract_id {
            Some(id) => id.clone(),
            None => continue,
        };

        // Check if first topic is "transfer"
        let first_topic = match event.topics.first() {
            Some(t) => t,
            None => continue,
        };

        let topic_str = extract_symbol_from_scval(first_topic);
        if topic_str.as_deref() != Some(SAC_TRANSFER_TOPIC) {
            continue;
        }

        // topics[1] = from, topics[2] = to, topics[3] = asset (optional)
        let from = extract_address_from_scval(event.topics.get(1));
        let to = extract_address_from_scval(event.topics.get(2));
        let asset_symbol = event.topics.get(3).and_then(extract_symbol_from_scval);

        // Amount is in event.data as i128
        let amount_raw = extract_amount_from_scval(&event.data).unwrap_or(0);

        let (direction, counterparty) = if from.as_deref() == Some(invoking_account) {
            (FlowDirection::Outbound, to)
        } else if to.as_deref() == Some(invoking_account) {
            (FlowDirection::Inbound, from)
        } else {
            // Not involving invoking account
            continue;
        };

        let decimals = DEFAULT_DECIMALS;
        let amount_display = format_amount(amount_raw, decimals);

        flows.push(AssetFlow {
            asset_id: contract_id,
            asset_symbol,
            direction,
            amount_raw,
            amount_display,
            decimals,
            counterparty,
        });
    }

    flows
}

/// Format a raw integer amount with the given decimal places.
pub fn format_amount(raw: u128, decimals: u8) -> String {
    if decimals == 0 {
        return raw.to_string();
    }
    let divisor = 10u128.pow(decimals as u32);
    let whole = raw / divisor;
    let frac = raw % divisor;
    format!("{}.{:0>width$}", whole, frac, width = decimals as usize)
}

/// Extract a string symbol/fn-name from an ScVal JSON representation.
/// Handles {"symbol": "transfer"} and {"string": "transfer"} forms.
pub fn extract_symbol_from_scval(val: &serde_json::Value) -> Option<String> {
    if let Some(s) = val.get("symbol").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = val.get("string").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    // Plain string
    if let Some(s) = val.as_str() {
        return Some(s.to_string());
    }
    None
}

/// Extract an address string from an ScVal JSON representation.
/// Handles {"address": "G..."} and {"accountId": "G..."} forms.
pub fn extract_address_from_scval(val: Option<&serde_json::Value>) -> Option<String> {
    let val = val?;
    if let Some(s) = val.get("address").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = val.get("accountId").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    if let Some(s) = val.as_str() {
        return Some(s.to_string());
    }
    None
}

/// Extract an i128/u128 amount from an ScVal JSON representation.
/// Handles {"i128": {"lo": N, "hi": N}}, {"u128": ...}, and plain numbers.
pub fn extract_amount_from_scval(val: &serde_json::Value) -> Option<u128> {
    // {"i128": {"lo": N, "hi": N}} — stellar-sdk representation
    if let Some(i128_obj) = val.get("i128") {
        let lo = i128_obj.get("lo").and_then(|v| v.as_u64()).unwrap_or(0) as u128;
        let hi = i128_obj.get("hi").and_then(|v| v.as_u64()).unwrap_or(0) as u128;
        return Some((hi << 64) | lo);
    }
    if let Some(u128_obj) = val.get("u128") {
        let lo = u128_obj.get("lo").and_then(|v| v.as_u64()).unwrap_or(0) as u128;
        let hi = u128_obj.get("hi").and_then(|v| v.as_u64()).unwrap_or(0) as u128;
        return Some((hi << 64) | lo);
    }
    // Plain number
    if let Some(n) = val.as_u64() {
        return Some(n as u128);
    }
    if let Some(n) = val.as_f64() {
        return Some(n as u128);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_amount_xlm() {
        assert_eq!(format_amount(50_000_000, 7), "5.0000000");
        assert_eq!(format_amount(1_234_567, 7), "0.1234567");
        assert_eq!(format_amount(0, 7), "0.0000000");
    }

    #[test]
    fn test_extract_symbol() {
        let val = serde_json::json!({"symbol": "transfer"});
        assert_eq!(extract_symbol_from_scval(&val), Some("transfer".to_string()));
    }

    #[test]
    fn test_extract_amount_i128() {
        let val = serde_json::json!({"i128": {"lo": 50000000, "hi": 0}});
        assert_eq!(extract_amount_from_scval(&val), Some(50_000_000));
    }

    #[test]
    fn test_extract_asset_flows_outbound() {
        let events = vec![DiagnosticEvent {
            in_successful_call: true,
            contract_id: Some("CUSDC_SAC".to_string()),
            event_type: "contract".to_string(),
            topics: vec![
                serde_json::json!({"symbol": "transfer"}),
                serde_json::json!({"address": "GABC_INVOKER"}),
                serde_json::json!({"address": "GXYZ_RECIPIENT"}),
                serde_json::json!({"symbol": "USDC"}),
            ],
            data: serde_json::json!({"i128": {"lo": 10_0000000u64, "hi": 0u64}}),
        }];

        let flows = extract_asset_flows(&events, "GABC_INVOKER");
        assert_eq!(flows.len(), 1);
        assert_eq!(flows[0].direction, FlowDirection::Outbound);
        assert_eq!(flows[0].asset_symbol, Some("USDC".to_string()));
        assert_eq!(flows[0].amount_raw, 1_000_000_000);
    }

    #[test]
    fn test_extract_asset_flows_ignores_unrelated() {
        let events = vec![DiagnosticEvent {
            in_successful_call: true,
            contract_id: Some("CUSDC_SAC".to_string()),
            event_type: "contract".to_string(),
            topics: vec![
                serde_json::json!({"symbol": "transfer"}),
                serde_json::json!({"address": "GOTHER1"}),
                serde_json::json!({"address": "GOTHER2"}),
            ],
            data: serde_json::json!(100),
        }];

        let flows = extract_asset_flows(&events, "GABC_INVOKER");
        assert_eq!(flows.len(), 0);
    }
}
