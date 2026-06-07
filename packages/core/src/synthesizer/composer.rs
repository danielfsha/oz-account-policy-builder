//! Compose existing OZ primitives into a PolicySpec.
//!
//! When composition_mode == Compose, this module is used to select
//! existing OZ contract addresses to wire together.

use crate::synthesizer::policy_spec::{PolicyLayer, PolicyLayerKind};

/// A reference to a deployed OZ primitive contract.
#[derive(Debug, Clone)]
pub struct OzPrimitive {
    /// Human-readable name
    pub name: &'static str,
    /// Policy layer kind this primitive implements
    pub kind: PolicyLayerKind,
    /// Description of what this primitive does
    pub description: &'static str,
    /// Install snippet fragment
    pub install_snippet: &'static str,
}

/// Known OZ account primitives.
pub const OZ_PRIMITIVES: &[OzPrimitive] = &[
    OzPrimitive {
        name: "SpendingLimit",
        kind: PolicyLayerKind::SpendingLimit,
        description: "Caps outbound token transfers per time window",
        install_snippet: "stellar contract invoke --id $POLICY_ID -- install --account $ACCOUNT --cap $CAP --window $WINDOW",
    },
    OzPrimitive {
        name: "TimeWindow",
        kind: PolicyLayerKind::TimeWindow,
        description: "Restricts contract calls to a specific time window",
        install_snippet: "stellar contract invoke --id $POLICY_ID -- install --account $ACCOUNT --window $WINDOW",
    },
    OzPrimitive {
        name: "SimpleThreshold",
        kind: PolicyLayerKind::SimpleThreshold,
        description: "Requires a single authorized signer",
        install_snippet: "stellar contract invoke --id $POLICY_ID -- install --account $ACCOUNT --threshold 1",
    },
    OzPrimitive {
        name: "WeightedThreshold",
        kind: PolicyLayerKind::WeightedThreshold,
        description: "Requires weighted multi-signer authorization",
        install_snippet: "stellar contract invoke --id $POLICY_ID -- install --account $ACCOUNT --threshold $THRESHOLD --signers $SIGNERS",
    },
];

/// Find the matching OZ primitive for a policy layer.
pub fn find_primitive(layer: &PolicyLayer) -> Option<&'static OzPrimitive> {
    OZ_PRIMITIVES.iter().find(|p| p.kind == layer.kind)
}

/// Generate a composition description for a set of policy layers.
pub fn describe_composition(layers: &[PolicyLayer]) -> String {
    let names: Vec<&str> = layers
        .iter()
        .filter(|l| l.oz_primitive)
        .filter_map(|l| find_primitive(l).map(|p| p.name))
        .collect();

    if names.is_empty() {
        "No composable OZ primitives — requires net-new codegen".to_string()
    } else {
        format!("Composing: {}", names.join(" + "))
    }
}
