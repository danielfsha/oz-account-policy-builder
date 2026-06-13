//! Walk and prune the call tree.
//!
//! Pruning rule: Keep a CallNode if:
//!   (a) the invoker is the invoking_account, OR
//!   (b) it has an asset delta involving the invoking_account.
//!
//! Everything else is discarded.

use crate::recorder::manifest::{AssetFlow, CallNode};

/// Prune a call tree to only nodes initiated by or relevant to the invoking account.
pub fn prune_call_tree(
    nodes: Vec<CallNode>,
    invoking_account: &str,
    asset_flows: &[AssetFlow],
) -> Vec<CallNode> {
    nodes
        .into_iter()
        .filter_map(|node| prune_node(node, invoking_account, asset_flows))
        .collect()
}

fn prune_node(
    mut node: CallNode,
    invoking_account: &str,
    asset_flows: &[AssetFlow],
) -> Option<CallNode> {
    // Rule (a): authorized by invoking account
    let authorized_by_invoker = node
        .authorized_by
        .as_deref()
        .map(|a| a == invoking_account)
        .unwrap_or(false);

    // Rule (b): has asset delta involving invoking account  
    let has_asset_delta = asset_flows.iter().any(|f| {
        f.counterparty.as_deref() == Some(invoking_account)
            || node.contract_id == f.asset_id
    });

    if !authorized_by_invoker && !has_asset_delta {
        return None;
    }

    // Recursively prune sub-calls
    node.sub_calls = prune_call_tree(
        std::mem::take(&mut node.sub_calls),
        invoking_account,
        asset_flows,
    );

    Some(node)
}

/// Collect all unique contract IDs from a call tree.
pub fn collect_unique_contracts(nodes: &[CallNode]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    collect_contracts_recursive(nodes, &mut seen, &mut result);
    result
}

fn collect_contracts_recursive(
    nodes: &[CallNode],
    seen: &mut std::collections::HashSet<String>,
    out: &mut Vec<String>,
) {
    for node in nodes {
        if seen.insert(node.contract_id.clone()) {
            out.push(node.contract_id.clone());
        }
        collect_contracts_recursive(&node.sub_calls, seen, out);
    }
}

/// Collect all unique function names from a call tree.
pub fn collect_unique_functions(nodes: &[CallNode]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    collect_functions_recursive(nodes, &mut seen, &mut result);
    result
}

fn collect_functions_recursive(
    nodes: &[CallNode],
    seen: &mut std::collections::HashSet<String>,
    out: &mut Vec<String>,
) {
    for node in nodes {
        let key = format!("{}::{}", node.contract_id, node.function_name);
        if seen.insert(key) {
            out.push(node.function_name.clone());
        }
        collect_functions_recursive(&node.sub_calls, seen, out);
    }
}

/// Compute max nesting depth of a call tree.
pub fn max_depth(nodes: &[CallNode]) -> usize {
    nodes
        .iter()
        .map(|n| 1 + max_depth(&n.sub_calls))
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::manifest::{AssetFlow, FlowDirection};

    fn make_node(contract_id: &str, function_name: &str, authorized_by: Option<&str>) -> CallNode {
        CallNode {
            contract_id: contract_id.to_string(),
            contract_label: None,
            function_name: function_name.to_string(),
            args: vec![],
            sub_calls: vec![],
            requires_auth: authorized_by.is_some(),
            authorized_by: authorized_by.map(String::from),
        }
    }

    #[test]
    fn test_prune_keeps_authorized_node() {
        let nodes = vec![make_node("C1", "swap", Some("GABC"))];
        let pruned = prune_call_tree(nodes, "GABC", &[]);
        assert_eq!(pruned.len(), 1);
    }

    #[test]
    fn test_prune_removes_unauthorized_node() {
        let nodes = vec![make_node("C1", "internal_fn", None)];
        let pruned = prune_call_tree(nodes, "GABC", &[]);
        assert_eq!(pruned.len(), 0);
    }

    #[test]
    fn test_prune_keeps_asset_flow_node() {
        let nodes = vec![make_node("C_SAC", "transfer", None)];
        let flows = vec![AssetFlow {
            asset_id: "C_SAC".to_string(),
            asset_symbol: Some("USDC".to_string()),
            direction: FlowDirection::Outbound,
            amount_raw: 1_000_000,
            amount_display: "1.0000000".to_string(),
            decimals: 7,
            counterparty: Some("GABC".to_string()),
        }];
        let pruned = prune_call_tree(nodes, "GABC", &flows);
        assert_eq!(pruned.len(), 1);
    }

    #[test]
    fn test_collect_unique_functions() {
        let mut root = make_node("C1", "swap", Some("GABC"));
        root.sub_calls = vec![make_node("C2", "transfer", None)];
        let fns = collect_unique_functions(&[root]);
        assert!(fns.contains(&"swap".to_string()));
        assert!(fns.contains(&"transfer".to_string()));
    }

    #[test]
    fn test_max_depth() {
        let mut root = make_node("C1", "swap", Some("GABC"));
        root.sub_calls = vec![make_node("C2", "transfer", None)];
        assert_eq!(max_depth(&[root]), 2);
    }
}
