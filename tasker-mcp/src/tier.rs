//! Tool tier configuration and resolution for profile-driven tool exposure.
//!
//! Controls which MCP tools are registered on the server based on profile
//! configuration, CLI overrides, and connectivity mode.

use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

/// A tool tier classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolTier {
    /// Offline developer tooling (template validation, codegen, schema tools)
    Tier1,
    /// Connected read-only tools (task/step inspection, DLQ, analytics, system)
    Tier2,
    /// Write tools with confirmation semantics (task submit/cancel, step operations, DLQ update)
    Tier3,
}

impl fmt::Display for ToolTier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToolTier::Tier1 => write!(f, "tier1"),
            ToolTier::Tier2 => write!(f, "tier2"),
            ToolTier::Tier3 => write!(f, "tier3"),
        }
    }
}

impl FromStr for ToolTier {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tier1" | "t1" => Ok(ToolTier::Tier1),
            "tier2" | "t2" => Ok(ToolTier::Tier2),
            "tier3" | "t3" => Ok(ToolTier::Tier3),
            _ => Err(format!(
                "Invalid tier '{}': expected 'tier1', 'tier2', or 'tier3'",
                s
            )),
        }
    }
}

/// Tier 2 connected read-only tool names.
pub const TIER2_TOOLS: &[&str] = &[
    "task_list",
    "task_inspect",
    "step_inspect",
    "step_audit",
    "dlq_list",
    "dlq_inspect",
    "dlq_stats",
    "dlq_queue",
    "staleness_check",
    "analytics_performance",
    "analytics_bottlenecks",
    "system_health",
    "system_config",
    "template_list_remote",
    "template_inspect_remote",
];

/// Tier 3 write tool names.
pub const TIER3_TOOLS: &[&str] = &[
    "task_submit",
    "task_cancel",
    "step_retry",
    "step_resolve",
    "step_complete",
    "dlq_update",
];

/// Set of enabled tool tiers controlling which tools are registered on the MCP server.
#[derive(Debug, Clone)]
pub struct EnabledTiers {
    tiers: HashSet<ToolTier>,
}

impl EnabledTiers {
    /// Parse tier strings from profile config or CLI.
    pub fn from_tier_strings(tiers: &[String]) -> Self {
        let mut set = HashSet::new();
        for s in tiers {
            match s.parse::<ToolTier>() {
                Ok(tier) => {
                    set.insert(tier);
                }
                Err(e) => {
                    tracing::warn!("{}", e);
                }
            }
        }
        // Always include Tier 1
        set.insert(ToolTier::Tier1);
        Self { tiers: set }
    }

    /// All tiers enabled.
    pub fn all() -> Self {
        let mut tiers = HashSet::new();
        tiers.insert(ToolTier::Tier1);
        tiers.insert(ToolTier::Tier2);
        tiers.insert(ToolTier::Tier3);
        Self { tiers }
    }

    /// Only Tier 1 enabled (offline mode).
    pub fn tier1_only() -> Self {
        let mut tiers = HashSet::new();
        tiers.insert(ToolTier::Tier1);
        Self { tiers }
    }

    /// Resolve which tiers to enable based on mode and configuration.
    ///
    /// Priority:
    /// 1. `--offline` → Tier 1 only
    /// 2. Explicit `tools` config → parse (warn if tier2/tier3 requested while offline)
    /// 3. No config + connected → all tiers
    pub fn resolve(offline: bool, profile_tools: Option<&[String]>) -> Self {
        if offline {
            if let Some(tools) = profile_tools {
                let requested: Vec<_> = tools
                    .iter()
                    .filter_map(|s| s.parse::<ToolTier>().ok())
                    .collect();
                let has_connected = requested
                    .iter()
                    .any(|t| matches!(t, ToolTier::Tier2 | ToolTier::Tier3));
                if has_connected {
                    tracing::warn!(
                        "Profile configures connected tiers (tier2/tier3) but running in offline mode; \
                         only tier1 tools will be available"
                    );
                }
            }
            return Self::tier1_only();
        }

        match profile_tools {
            Some(tools) if !tools.is_empty() => Self::from_tier_strings(tools),
            _ => Self::all(),
        }
    }

    /// Check if a tier is enabled.
    pub fn includes(&self, tier: ToolTier) -> bool {
        self.tiers.contains(&tier)
    }

    /// Return tool names that should be removed from the router based on disabled tiers.
    pub fn tools_to_remove(&self) -> Vec<&'static str> {
        let mut names = Vec::new();
        if !self.includes(ToolTier::Tier2) {
            names.extend_from_slice(TIER2_TOOLS);
        }
        if !self.includes(ToolTier::Tier3) {
            names.extend_from_slice(TIER3_TOOLS);
        }
        names
    }

    /// Human-readable description of enabled tiers for logging and instructions.
    pub fn description(&self) -> String {
        let mut parts = Vec::new();
        if self.includes(ToolTier::Tier1) {
            parts.push("tier1 (developer tooling)");
        }
        if self.includes(ToolTier::Tier2) {
            parts.push("tier2 (read-only)");
        }
        if self.includes(ToolTier::Tier3) {
            parts.push("tier3 (write)");
        }
        parts.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_tier_from_str() {
        assert_eq!("tier1".parse::<ToolTier>().unwrap(), ToolTier::Tier1);
        assert_eq!("tier2".parse::<ToolTier>().unwrap(), ToolTier::Tier2);
        assert_eq!("tier3".parse::<ToolTier>().unwrap(), ToolTier::Tier3);
        assert_eq!("t1".parse::<ToolTier>().unwrap(), ToolTier::Tier1);
        assert_eq!("TIER2".parse::<ToolTier>().unwrap(), ToolTier::Tier2);
        assert!("invalid".parse::<ToolTier>().is_err());
    }

    #[test]
    fn test_tool_tier_display() {
        assert_eq!(ToolTier::Tier1.to_string(), "tier1");
        assert_eq!(ToolTier::Tier2.to_string(), "tier2");
        assert_eq!(ToolTier::Tier3.to_string(), "tier3");
    }

    #[test]
    fn test_enabled_tiers_all() {
        let tiers = EnabledTiers::all();
        assert!(tiers.includes(ToolTier::Tier1));
        assert!(tiers.includes(ToolTier::Tier2));
        assert!(tiers.includes(ToolTier::Tier3));
        assert!(tiers.tools_to_remove().is_empty());
    }

    #[test]
    fn test_enabled_tiers_tier1_only() {
        let tiers = EnabledTiers::tier1_only();
        assert!(tiers.includes(ToolTier::Tier1));
        assert!(!tiers.includes(ToolTier::Tier2));
        assert!(!tiers.includes(ToolTier::Tier3));

        let removed = tiers.tools_to_remove();
        assert_eq!(removed.len(), TIER2_TOOLS.len() + TIER3_TOOLS.len());
    }

    #[test]
    fn test_from_tier_strings_always_includes_tier1() {
        let tiers = EnabledTiers::from_tier_strings(&["tier2".to_string()]);
        assert!(tiers.includes(ToolTier::Tier1));
        assert!(tiers.includes(ToolTier::Tier2));
        assert!(!tiers.includes(ToolTier::Tier3));
    }

    #[test]
    fn test_from_tier_strings_invalid_ignored() {
        let tiers = EnabledTiers::from_tier_strings(&["tier1".to_string(), "bogus".to_string()]);
        assert!(tiers.includes(ToolTier::Tier1));
        assert!(!tiers.includes(ToolTier::Tier2));
    }

    #[test]
    fn test_resolve_offline_forces_tier1() {
        let tiers = EnabledTiers::resolve(true, None);
        assert!(tiers.includes(ToolTier::Tier1));
        assert!(!tiers.includes(ToolTier::Tier2));
        assert!(!tiers.includes(ToolTier::Tier3));
    }

    #[test]
    fn test_resolve_offline_ignores_profile_tools() {
        let tools = vec![
            "tier1".to_string(),
            "tier2".to_string(),
            "tier3".to_string(),
        ];
        let tiers = EnabledTiers::resolve(true, Some(&tools));
        assert!(tiers.includes(ToolTier::Tier1));
        assert!(!tiers.includes(ToolTier::Tier2));
        assert!(!tiers.includes(ToolTier::Tier3));
    }

    #[test]
    fn test_resolve_connected_no_config_enables_all() {
        let tiers = EnabledTiers::resolve(false, None);
        assert!(tiers.includes(ToolTier::Tier1));
        assert!(tiers.includes(ToolTier::Tier2));
        assert!(tiers.includes(ToolTier::Tier3));
    }

    #[test]
    fn test_resolve_connected_with_config() {
        let tools = vec!["tier1".to_string(), "tier2".to_string()];
        let tiers = EnabledTiers::resolve(false, Some(&tools));
        assert!(tiers.includes(ToolTier::Tier1));
        assert!(tiers.includes(ToolTier::Tier2));
        assert!(!tiers.includes(ToolTier::Tier3));
    }

    #[test]
    fn test_tools_to_remove_tier1_tier2() {
        let tools = vec!["tier1".to_string(), "tier2".to_string()];
        let tiers = EnabledTiers::from_tier_strings(&tools);
        let removed = tiers.tools_to_remove();
        // Should remove only Tier 3 tools
        assert_eq!(removed.len(), TIER3_TOOLS.len());
        for tool in TIER3_TOOLS {
            assert!(removed.contains(tool));
        }
    }

    #[test]
    fn test_tier2_tools_count() {
        assert_eq!(TIER2_TOOLS.len(), 15);
    }

    #[test]
    fn test_tier3_tools_count() {
        assert_eq!(TIER3_TOOLS.len(), 6);
    }

    #[test]
    fn test_description() {
        let tiers = EnabledTiers::all();
        let desc = tiers.description();
        assert!(desc.contains("tier1"));
        assert!(desc.contains("tier2"));
        assert!(desc.contains("tier3"));

        let tier1_only = EnabledTiers::tier1_only();
        let desc = tier1_only.description();
        assert!(desc.contains("tier1"));
        assert!(!desc.contains("tier2"));
    }
}
