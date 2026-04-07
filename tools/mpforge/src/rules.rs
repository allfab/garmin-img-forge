//! Rules file parsing and validation for attribute transformation.
//!
//! Story 9.1: Loads and validates a YAML rules file that defines
//! how source attributes are transformed for Polish Map export.

use anyhow::Context;
use regex::Regex;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::LazyLock;
use tracing::{info, warn};

/// Top-level rules file structure.
/// Corresponds to the YAML file with `version` and `rulesets`.
#[derive(Debug, Deserialize)]
pub struct RulesFile {
    pub version: u32,
    pub rulesets: Vec<Ruleset>,
}

/// A ruleset targeting a specific source layer.
#[derive(Debug, Deserialize)]
pub struct Ruleset {
    pub name: Option<String>,
    pub source_layer: String,
    pub rules: Vec<Rule>,
}

/// A single transformation rule with match conditions and set values.
#[derive(Debug, Deserialize)]
pub struct Rule {
    /// Match conditions: field_name → pattern.
    /// Renamed from YAML `match` (Rust reserved keyword).
    #[serde(rename = "match")]
    pub match_conditions: HashMap<String, String>,
    /// Target attributes to set: attribute_name → value_or_template.
    pub set: HashMap<String, String>,
}

/// Domain errors for rule evaluation.
#[derive(Debug, thiserror::Error)]
pub enum RuleError {
    /// Type field value doesn't match the required hex format.
    #[error("Invalid Type value '{value}': must match 0x[0-9a-fA-F]+")]
    InvalidTypeHex { value: String },
}

/// Per-ruleset statistics for rule evaluation.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RulesetStats {
    pub matched: usize,
    pub ignored: usize,
    pub errors: usize,
}

/// Aggregated statistics for all rule evaluations across the pipeline.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct RuleStats {
    pub matched: usize,
    pub ignored: usize,
    pub errors: usize,
    pub by_ruleset: BTreeMap<String, RulesetStats>,
}

impl RuleStats {
    pub fn record_match(&mut self, layer_name: &str) {
        self.matched += 1;
        let entry = self.by_ruleset.entry(layer_name.to_string()).or_default();
        entry.matched += 1;
    }

    pub fn record_ignored(&mut self, layer_name: &str) {
        self.ignored += 1;
        let entry = self.by_ruleset.entry(layer_name.to_string()).or_default();
        entry.ignored += 1;
    }

    pub fn record_error(&mut self, layer_name: &str) {
        self.errors += 1;
        let entry = self.by_ruleset.entry(layer_name.to_string()).or_default();
        entry.errors += 1;
    }
}

/// Pre-compiled regex for `${FIELD}` substitution patterns.
static SUBST_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\$\{([^}]+)\}").expect("valid regex"));

/// Pre-compiled regex for Type hex validation (`0x[0-9a-fA-F]+`).
static TYPE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^0x[0-9a-fA-F]+$").expect("valid regex"));

/// Load and validate a rules file from disk.
pub fn load_rules(path: &Path) -> anyhow::Result<RulesFile> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read rules file: {}", path.display()))?;

    let rules_file: RulesFile = serde_yml::from_str(&content)
        .with_context(|| format!("Failed to parse YAML rules file: {}", path.display()))?;

    validate_rules(&rules_file)?;

    let total_rules: usize = rules_file.rulesets.iter().map(|rs| rs.rules.len()).sum();
    info!(
        rulesets = rules_file.rulesets.len(),
        total_rules, "Rules file loaded successfully"
    );

    Ok(rules_file)
}

/// Validate semantic rules after parsing.
fn validate_rules(rules_file: &RulesFile) -> anyhow::Result<()> {
    if rules_file.rulesets.is_empty() {
        anyhow::bail!("Rules file must contain at least one ruleset");
    }

    if rules_file.version != 1 {
        anyhow::bail!(
            "Unsupported rules file version: {}, expected 1",
            rules_file.version
        );
    }

    for (i, ruleset) in rules_file.rulesets.iter().enumerate() {
        let default_name = format!("#{}", i + 1);
        let ruleset_name = ruleset.name.as_deref().unwrap_or(&default_name);

        if ruleset.source_layer.is_empty() {
            anyhow::bail!(
                "Ruleset '{}': missing source_layer",
                ruleset_name
            );
        }

        if ruleset.rules.is_empty() {
            anyhow::bail!(
                "Ruleset '{}': at least one rule is required",
                ruleset_name
            );
        }

        for (j, rule) in ruleset.rules.iter().enumerate() {
            if rule.match_conditions.is_empty() {
                anyhow::bail!(
                    "Ruleset '{}', rule #{}: missing match (at least one condition required)",
                    ruleset_name,
                    j + 1
                );
            }

            if rule.set.is_empty() {
                anyhow::bail!(
                    "Ruleset '{}', rule #{}: missing set (at least one target attribute required)",
                    ruleset_name,
                    j + 1
                );
            }
        }
    }

    Ok(())
}

/// Find the ruleset matching a given source layer name.
///
/// Story 9.3 - Task 2: Case-sensitive lookup by `source_layer` field.
/// Returns the first matching ruleset (consistent with GDAL layer names).
pub fn find_ruleset<'a>(rules: &'a RulesFile, layer_name: &str) -> Option<&'a Ruleset> {
    rules
        .rulesets
        .iter()
        .find(|rs| rs.source_layer == layer_name)
}

/// Evaluate match conditions against feature attributes.
///
/// All conditions must be satisfied (AND logic). A missing field
/// in feature attributes is treated as an empty string.
///
/// Pattern operators (checked in order):
/// - `"*"` → wildcard (always matches)
/// - `"!!"` → field is present and non-empty
/// - `""` → field is absent or empty
/// - `"in:v1,v2,v3"` → value is one of the comma-separated values
/// - `"!in:v1,v2,v3"` → value is NOT one of the comma-separated values
/// - `"^prefix"` → value starts with prefix (case-sensitive)
/// - `"^i:prefix"` → value starts with prefix (case-insensitive)
/// - `"!^prefix"` → value does NOT start with prefix (case-sensitive)
/// - `"!^i:prefix"` → value does NOT start with prefix (case-insensitive)
/// - `"!<value>"` → not equal to `<value>`
/// - anything else → strict equality
pub fn evaluate_match(
    match_conditions: &HashMap<String, String>,
    feature_attrs: &HashMap<String, String>,
) -> bool {
    for (field, pattern) in match_conditions {
        let attr_value = feature_attrs
            .get(field)
            .map(|s| s.as_str())
            .unwrap_or("");

        let matches = if pattern == "*" {
            // Wildcard: always true
            true
        } else if pattern == "!!" {
            // Non-empty: value must be present and non-empty
            !attr_value.is_empty()
        } else if pattern.is_empty() {
            // Empty: value must be absent or empty
            attr_value.is_empty()
        } else if let Some(list) = pattern.strip_prefix("in:") {
            // In-list: value must be one of the comma-separated values
            list.split(',').any(|v| v.trim() == attr_value)
        } else if let Some(list) = pattern.strip_prefix("!in:") {
            // Not-in-list: value must NOT be one of the comma-separated values
            !list.split(',').any(|v| v.trim() == attr_value)
        } else if let Some(prefix) = pattern.strip_prefix("!^i:") {
            // Not-starts-with case-insensitive
            !attr_value.to_lowercase().starts_with(&prefix.to_lowercase())
        } else if let Some(prefix) = pattern.strip_prefix("!^") {
            // Not-starts-with case-sensitive
            !attr_value.starts_with(prefix)
        } else if let Some(prefix) = pattern.strip_prefix("^i:") {
            // Starts-with case-insensitive
            attr_value.to_lowercase().starts_with(&prefix.to_lowercase())
        } else if let Some(prefix) = pattern.strip_prefix('^') {
            // Starts-with case-sensitive
            attr_value.starts_with(prefix)
        } else if pattern.starts_with('!') && pattern.len() > 1 {
            // Not equal: strip '!' prefix and compare
            attr_value != &pattern[1..]
        } else {
            // Strict equality
            attr_value == pattern
        };

        if !matches {
            return false;
        }
    }
    true
}

/// Apply set transformations to produce output attributes.
///
/// Substitutes `${FIELD}` patterns with values from feature attributes.
/// Missing fields produce an empty string. Validates that `Type` values
/// match `^0x[0-9a-fA-F]+$`.
pub fn apply_set(
    set: &HashMap<String, String>,
    feature_attrs: &HashMap<String, String>,
) -> Result<HashMap<String, String>, RuleError> {
    let mut result = HashMap::new();
    for (attr_name, template) in set {
        let value = SUBST_RE
            .replace_all(template, |caps: &regex::Captures| {
                let field_name = &caps[1];
                feature_attrs
                    .get(field_name)
                    .cloned()
                    .unwrap_or_default()
            })
            .to_string();

        // Validate Type field
        if attr_name == "Type" && !TYPE_RE.is_match(&value) {
            warn!(value = %value, "Invalid Type hex value");
            return Err(RuleError::InvalidTypeHex { value });
        }

        result.insert(attr_name.clone(), value);
    }
    Ok(result)
}

/// Evaluate a feature against all rules in a ruleset (first-match-wins).
///
/// Returns `Some(transformed_attrs)` for the first matching rule,
/// or `None` if no rule matches.
pub fn evaluate_feature(
    ruleset: &Ruleset,
    feature_attrs: &HashMap<String, String>,
) -> Result<Option<HashMap<String, String>>, RuleError> {
    for rule in &ruleset.rules {
        if evaluate_match(&rule.match_conditions, feature_attrs) {
            let transformed = apply_set(&rule.set, feature_attrs)?;
            return Ok(Some(transformed));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_minimal_rules() {
        let yaml = r#"
version: 1
rulesets:
  - name: "Routes"
    source_layer: "TRONCON_DE_ROUTE"
    rules:
      - match:
          CL_ADMIN: "Autoroute"
        set:
          Type: "0x01"
"#;
        let rules: RulesFile = serde_yml::from_str(yaml).unwrap();
        assert_eq!(rules.version, 1);
        assert_eq!(rules.rulesets.len(), 1);
        assert_eq!(rules.rulesets[0].source_layer, "TRONCON_DE_ROUTE");
        assert_eq!(rules.rulesets[0].rules.len(), 1);
        assert_eq!(
            rules.rulesets[0].rules[0].match_conditions.get("CL_ADMIN"),
            Some(&"Autoroute".to_string())
        );
        assert_eq!(
            rules.rulesets[0].rules[0].set.get("Type"),
            Some(&"0x01".to_string())
        );
        assert!(validate_rules(&rules).is_ok());
    }

    #[test]
    fn test_parse_complete_rules_multi_rulesets() {
        let yaml = r#"
version: 1
rulesets:
  - name: "Routes"
    source_layer: "TRONCON_DE_ROUTE"
    rules:
      - match:
          CL_ADMIN: "Autoroute"
        set:
          Type: "0x01"
          EndLevel: "1"
          Label: "${NUMERO}"
      - match:
          CL_ADMIN: "Nationale"
          NATURE: "!Rond-point"
        set:
          Type: "0x02"
          EndLevel: "2"
          Label: "${NUMERO}"
      - match:
          CL_ADMIN: "Départementale"
        set:
          Type: "0x03"
      - match:
          CL_ADMIN: "Communale"
        set:
          Type: "0x06"
      - match:
          CL_ADMIN: "Chemin"
        set:
          Type: "0x07"
  - name: "Hydro"
    source_layer: "COURS_D_EAU"
    rules:
      - match:
          REGIME: "Permanent"
        set:
          Type: "0x1f"
          Label: "${TOPONYME}"
      - match:
          REGIME: "Intermittent"
        set:
          Type: "0x26"
      - match:
          REGIME: "Temporaire"
        set:
          Type: "0x26"
  - name: "Bati"
    source_layer: "BATIMENT"
    rules:
      - match:
          NATURE: "Bâtiment remarquable"
        set:
          Type: "0x13"
          Label: "${TOPONYME}"
      - match:
          NATURE: "Bâtiment industriel"
        set:
          Type: "0x0a"
"#;
        let rules: RulesFile = serde_yml::from_str(yaml).unwrap();
        assert_eq!(rules.rulesets.len(), 3);
        assert_eq!(rules.rulesets[0].rules.len(), 5);
        assert_eq!(rules.rulesets[1].rules.len(), 3);
        assert_eq!(rules.rulesets[2].rules.len(), 2);
        // Multi-match AND (2 conditions)
        assert_eq!(rules.rulesets[0].rules[1].match_conditions.len(), 2);
        assert!(validate_rules(&rules).is_ok());
    }

    #[test]
    fn test_error_malformed_yaml() {
        let yaml = r#"
version: 1
rulesets:
  - name: "Bad
    source_layer: "broken
"#;
        let result = serde_yml::from_str::<RulesFile>(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_ruleset_no_source_layer() {
        let yaml = r#"
version: 1
rulesets:
  - name: "Routes"
    source_layer: ""
    rules:
      - match:
          CL_ADMIN: "Autoroute"
        set:
          Type: "0x01"
"#;
        let rules: RulesFile = serde_yml::from_str(yaml).unwrap();
        let result = validate_rules(&rules);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("missing source_layer"));
    }

    #[test]
    fn test_error_rule_no_set() {
        let yaml = r#"
version: 1
rulesets:
  - name: "Routes"
    source_layer: "TRONCON_DE_ROUTE"
    rules:
      - match:
          CL_ADMIN: "Autoroute"
        set: {}
"#;
        let rules: RulesFile = serde_yml::from_str(yaml).unwrap();
        let result = validate_rules(&rules);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing set"));
    }

    #[test]
    fn test_error_rule_no_match() {
        let yaml = r#"
version: 1
rulesets:
  - name: "Routes"
    source_layer: "TRONCON_DE_ROUTE"
    rules:
      - match: {}
        set:
          Type: "0x01"
"#;
        let rules: RulesFile = serde_yml::from_str(yaml).unwrap();
        let result = validate_rules(&rules);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("missing match"));
    }

    #[test]
    fn test_error_empty_rules_list() {
        let yaml = r#"
version: 1
rulesets:
  - name: "Routes"
    source_layer: "TRONCON_DE_ROUTE"
    rules: []
"#;
        let rules: RulesFile = serde_yml::from_str(yaml).unwrap();
        let result = validate_rules(&rules);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("at least one rule is required"));
    }

    #[test]
    fn test_error_empty_rulesets() {
        let yaml = r#"
version: 1
rulesets: []
"#;
        let rules: RulesFile = serde_yml::from_str(yaml).unwrap();
        let result = validate_rules(&rules);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("at least one ruleset"));
    }

    #[test]
    fn test_load_rules_full_path() {
        use std::io::Write;
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test_rules.yaml");
        let mut f = std::fs::File::create(&file_path).unwrap();
        write!(f, r#"version: 1
rulesets:
  - name: "Test"
    source_layer: "LAYER"
    rules:
      - match:
          FIELD: "value"
        set:
          Type: "0x01"
"#).unwrap();
        let result = load_rules(&file_path);
        assert!(result.is_ok());
        let rules_file = result.unwrap();
        assert_eq!(rules_file.rulesets.len(), 1);
        assert_eq!(rules_file.rulesets[0].source_layer, "LAYER");
    }

    // ===================================================================
    // Task 4 — Tests evaluate_match (AC: 1,2,3,4,5,6)
    // ===================================================================

    #[test]
    fn test_match_strict_equality_matches() {
        let conditions = HashMap::from([("NATURE".into(), "Autoroute".into())]);
        let attrs = HashMap::from([("NATURE".into(), "Autoroute".into())]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_strict_equality_no_match() {
        let conditions = HashMap::from([("NATURE".into(), "Autoroute".into())]);
        let attrs = HashMap::from([("NATURE".into(), "Nationale".into())]);
        assert!(!evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_not_equal_matches() {
        let conditions = HashMap::from([("NATURE".into(), "!Rond-point".into())]);
        let attrs = HashMap::from([("NATURE".into(), "Sentier".into())]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_not_equal_no_match() {
        let conditions = HashMap::from([("NATURE".into(), "!Rond-point".into())]);
        let attrs = HashMap::from([("NATURE".into(), "Rond-point".into())]);
        assert!(!evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_empty_matches_absent() {
        let conditions = HashMap::from([("CL_ADMIN".into(), "".into())]);
        let attrs = HashMap::new();
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_empty_matches_empty_string() {
        let conditions = HashMap::from([("CL_ADMIN".into(), "".into())]);
        let attrs = HashMap::from([("CL_ADMIN".into(), "".into())]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_empty_no_match_non_empty() {
        let conditions = HashMap::from([("CL_ADMIN".into(), "".into())]);
        let attrs = HashMap::from([("CL_ADMIN".into(), "Nationale".into())]);
        assert!(!evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_non_empty_matches() {
        let conditions = HashMap::from([("TOPONYME".into(), "!!".into())]);
        let attrs = HashMap::from([("TOPONYME".into(), "Mont Blanc".into())]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_non_empty_no_match_empty() {
        let conditions = HashMap::from([("TOPONYME".into(), "!!".into())]);
        let attrs = HashMap::from([("TOPONYME".into(), "".into())]);
        assert!(!evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_non_empty_no_match_absent() {
        let conditions = HashMap::from([("TOPONYME".into(), "!!".into())]);
        let attrs = HashMap::new();
        assert!(!evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_wildcard_matches_value() {
        let conditions = HashMap::from([("POS_SOL".into(), "*".into())]);
        let attrs = HashMap::from([("POS_SOL".into(), "2".into())]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_wildcard_matches_empty() {
        let conditions = HashMap::from([("POS_SOL".into(), "*".into())]);
        let attrs = HashMap::from([("POS_SOL".into(), "".into())]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_wildcard_matches_absent() {
        let conditions = HashMap::from([("POS_SOL".into(), "*".into())]);
        let attrs = HashMap::new();
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_multi_field_and_all_match() {
        let conditions = HashMap::from([
            ("CL_ADMIN".into(), "Nationale".into()),
            ("NATURE".into(), "!Rond-point".into()),
        ]);
        let attrs = HashMap::from([
            ("CL_ADMIN".into(), "Nationale".into()),
            ("NATURE".into(), "Route a 1 chaussee".into()),
        ]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_multi_field_and_one_fails() {
        let conditions = HashMap::from([
            ("CL_ADMIN".into(), "Nationale".into()),
            ("NATURE".into(), "!Rond-point".into()),
        ]);
        let attrs = HashMap::from([
            ("CL_ADMIN".into(), "Nationale".into()),
            ("NATURE".into(), "Rond-point".into()),
        ]);
        assert!(!evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_missing_field_treated_as_empty() {
        let conditions = HashMap::from([("MISSING".into(), "".into())]);
        let attrs = HashMap::from([("OTHER".into(), "value".into())]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_missing_field_strict_no_match() {
        let conditions = HashMap::from([("MISSING".into(), "something".into())]);
        let attrs = HashMap::new();
        assert!(!evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_not_equal_matches_absent_field() {
        // Absent field = "" → "" != "Rond-point" → should match
        let conditions = HashMap::from([("NATURE".into(), "!Rond-point".into())]);
        let attrs = HashMap::new();
        assert!(evaluate_match(&conditions, &attrs));
    }

    // ===================================================================
    // Tests: in-list operator (in:v1,v2,v3)
    // ===================================================================

    #[test]
    fn test_match_in_list_matches() {
        let conditions = HashMap::from([("IMPORTANCE".into(), "in:1,2,3".into())]);
        let attrs = HashMap::from([("IMPORTANCE".into(), "2".into())]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_in_list_first_value() {
        let conditions = HashMap::from([("IMPORTANCE".into(), "in:1,2,3".into())]);
        let attrs = HashMap::from([("IMPORTANCE".into(), "1".into())]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_in_list_last_value() {
        let conditions = HashMap::from([("IMPORTANCE".into(), "in:1,2,3".into())]);
        let attrs = HashMap::from([("IMPORTANCE".into(), "3".into())]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_in_list_no_match() {
        let conditions = HashMap::from([("IMPORTANCE".into(), "in:1,2,3".into())]);
        let attrs = HashMap::from([("IMPORTANCE".into(), "5".into())]);
        assert!(!evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_in_list_absent_field() {
        let conditions = HashMap::from([("IMPORTANCE".into(), "in:1,2,3".into())]);
        let attrs = HashMap::new();
        assert!(!evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_in_list_with_spaces() {
        let conditions = HashMap::from([("NATURE".into(), "in:Lieu-dit habité, Quartier, Ruines".into())]);
        let attrs = HashMap::from([("NATURE".into(), "Quartier".into())]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_not_in_list_matches() {
        let conditions = HashMap::from([("IMPORTANCE".into(), "!in:4,5".into())]);
        let attrs = HashMap::from([("IMPORTANCE".into(), "1".into())]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_not_in_list_no_match() {
        let conditions = HashMap::from([("IMPORTANCE".into(), "!in:4,5".into())]);
        let attrs = HashMap::from([("IMPORTANCE".into(), "4".into())]);
        assert!(!evaluate_match(&conditions, &attrs));
    }

    // ===================================================================
    // Tests: starts-with operators (^, ^i:, !^, !^i:)
    // ===================================================================

    #[test]
    fn test_match_starts_with_matches() {
        let conditions = HashMap::from([("TOPONYME".into(), "^Commune".into())]);
        let attrs = HashMap::from([("TOPONYME".into(), "Commune de Paris".into())]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_starts_with_no_match() {
        let conditions = HashMap::from([("TOPONYME".into(), "^Commune".into())]);
        let attrs = HashMap::from([("TOPONYME".into(), "Ville de Lyon".into())]);
        assert!(!evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_starts_with_case_sensitive() {
        let conditions = HashMap::from([("TOPONYME".into(), "^Commune".into())]);
        let attrs = HashMap::from([("TOPONYME".into(), "commune de Paris".into())]);
        assert!(!evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_starts_with_case_insensitive_matches() {
        let conditions = HashMap::from([("TOPONYME".into(), "^i:Commune d".into())]);
        let attrs = HashMap::from([("TOPONYME".into(), "commune de Paris".into())]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_starts_with_case_insensitive_no_match() {
        let conditions = HashMap::from([("TOPONYME".into(), "^i:Commune d".into())]);
        let attrs = HashMap::from([("TOPONYME".into(), "Ville de Lyon".into())]);
        assert!(!evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_not_starts_with_matches() {
        let conditions = HashMap::from([("TOPONYME".into(), "!^Commune".into())]);
        let attrs = HashMap::from([("TOPONYME".into(), "Ville de Lyon".into())]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_not_starts_with_no_match() {
        let conditions = HashMap::from([("TOPONYME".into(), "!^Commune".into())]);
        let attrs = HashMap::from([("TOPONYME".into(), "Commune de Paris".into())]);
        assert!(!evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_not_starts_with_case_insensitive_matches() {
        let conditions = HashMap::from([("TOPONYME".into(), "!^i:Commune d".into())]);
        let attrs = HashMap::from([("TOPONYME".into(), "Le Hameau".into())]);
        assert!(evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_not_starts_with_case_insensitive_no_match() {
        let conditions = HashMap::from([("TOPONYME".into(), "!^i:Commune d".into())]);
        let attrs = HashMap::from([("TOPONYME".into(), "COMMUNE DE PARIS".into())]);
        assert!(!evaluate_match(&conditions, &attrs));
    }

    #[test]
    fn test_match_not_starts_with_case_insensitive_absent_field() {
        // Absent field = "" → "" doesn't start with "Commune d" → true
        let conditions = HashMap::from([("TOPONYME".into(), "!^i:Commune d".into())]);
        let attrs = HashMap::new();
        assert!(evaluate_match(&conditions, &attrs));
    }

    // ===================================================================
    // Integration: FME-style multi-condition filter
    // ===================================================================

    #[test]
    fn test_match_fme_zone_habitation_filter() {
        // Reproduces: IMPORTANCE in {1,2,3} AND NATURE = "Lieu-dit habité" AND NOT TOPONYME starts with "Commune d" (case-insensitive)
        let conditions = HashMap::from([
            ("IMPORTANCE".into(), "in:1,2,3".into()),
            ("NATURE".into(), "Lieu-dit habité".into()),
            ("TOPONYME".into(), "!^i:Commune d".into()),
        ]);

        // Should match: all conditions satisfied
        let attrs_ok = HashMap::from([
            ("IMPORTANCE".into(), "2".into()),
            ("NATURE".into(), "Lieu-dit habité".into()),
            ("TOPONYME".into(), "Le Hameau".into()),
        ]);
        assert!(evaluate_match(&conditions, &attrs_ok));

        // Should NOT match: IMPORTANCE=5 not in list
        let attrs_bad_importance = HashMap::from([
            ("IMPORTANCE".into(), "5".into()),
            ("NATURE".into(), "Lieu-dit habité".into()),
            ("TOPONYME".into(), "Le Hameau".into()),
        ]);
        assert!(!evaluate_match(&conditions, &attrs_bad_importance));

        // Should NOT match: wrong NATURE
        let attrs_bad_nature = HashMap::from([
            ("IMPORTANCE".into(), "1".into()),
            ("NATURE".into(), "Quartier".into()),
            ("TOPONYME".into(), "Le Hameau".into()),
        ]);
        assert!(!evaluate_match(&conditions, &attrs_bad_nature));

        // Should NOT match: TOPONYME starts with "Commune d"
        let attrs_bad_toponyme = HashMap::from([
            ("IMPORTANCE".into(), "1".into()),
            ("NATURE".into(), "Lieu-dit habité".into()),
            ("TOPONYME".into(), "Commune de Paris".into()),
        ]);
        assert!(!evaluate_match(&conditions, &attrs_bad_toponyme));
    }

    // ===================================================================
    // Task 5 — Tests apply_set (AC: 7,8,9,10)
    // ===================================================================

    #[test]
    fn test_set_simple_substitution() {
        let set = HashMap::from([("Label".into(), "${NUMERO}".into())]);
        let attrs = HashMap::from([("NUMERO".into(), "D1075".into())]);
        let result = apply_set(&set, &attrs).unwrap();
        assert_eq!(result.get("Label").unwrap(), "D1075");
    }

    #[test]
    fn test_set_concatenation() {
        let set = HashMap::from([("Label".into(), "Ligne ${VOLTAGE}".into())]);
        let attrs = HashMap::from([("VOLTAGE".into(), "400kV".into())]);
        let result = apply_set(&set, &attrs).unwrap();
        assert_eq!(result.get("Label").unwrap(), "Ligne 400kV");
    }

    #[test]
    fn test_set_multi_substitution() {
        let set = HashMap::from([("Label".into(), "${CL_ADMIN} - ${NUMERO}".into())]);
        let attrs = HashMap::from([
            ("CL_ADMIN".into(), "Nationale".into()),
            ("NUMERO".into(), "D1075".into()),
        ]);
        let result = apply_set(&set, &attrs).unwrap();
        assert_eq!(result.get("Label").unwrap(), "Nationale - D1075");
    }

    #[test]
    fn test_set_missing_field_substitution() {
        let set = HashMap::from([("Label".into(), "${MISSING}".into())]);
        let attrs = HashMap::new();
        let result = apply_set(&set, &attrs).unwrap();
        assert_eq!(result.get("Label").unwrap(), "");
    }

    #[test]
    fn test_set_static_value_no_substitution() {
        let set = HashMap::from([("Type".into(), "0x01".into())]);
        let attrs = HashMap::new();
        let result = apply_set(&set, &attrs).unwrap();
        assert_eq!(result.get("Type").unwrap(), "0x01");
    }

    #[test]
    fn test_set_type_hex_valid_values() {
        for val in &["0x01", "0x1f", "0xFF", "0xABCD"] {
            let set = HashMap::from([("Type".into(), val.to_string())]);
            let result = apply_set(&set, &HashMap::new());
            assert!(result.is_ok(), "Expected valid for {}", val);
        }
    }

    #[test]
    fn test_set_type_hex_invalid_values() {
        for val in &["invalid", "0xZZ", "01", "x01", ""] {
            let set = HashMap::from([("Type".into(), val.to_string())]);
            let result = apply_set(&set, &HashMap::new());
            assert!(result.is_err(), "Expected error for '{}'", val);
        }
    }

    // ===================================================================
    // Task 6 — Tests evaluate_feature (AC: 1-10)
    // ===================================================================

    fn make_ruleset(rules: Vec<(HashMap<String, String>, HashMap<String, String>)>) -> Ruleset {
        Ruleset {
            name: Some("Test".into()),
            source_layer: "LAYER".into(),
            rules: rules
                .into_iter()
                .map(|(m, s)| Rule {
                    match_conditions: m,
                    set: s,
                })
                .collect(),
        }
    }

    #[test]
    fn test_evaluate_feature_first_rule_matches() {
        let ruleset = make_ruleset(vec![
            (
                HashMap::from([("NATURE".into(), "Autoroute".into())]),
                HashMap::from([("Type".into(), "0x01".into())]),
            ),
            (
                HashMap::from([("NATURE".into(), "Nationale".into())]),
                HashMap::from([("Type".into(), "0x02".into())]),
            ),
        ]);
        let attrs = HashMap::from([("NATURE".into(), "Autoroute".into())]);
        let result = evaluate_feature(&ruleset, &attrs).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().get("Type").unwrap(), "0x01");
    }

    #[test]
    fn test_evaluate_feature_second_rule_matches() {
        let ruleset = make_ruleset(vec![
            (
                HashMap::from([("NATURE".into(), "Autoroute".into())]),
                HashMap::from([("Type".into(), "0x01".into())]),
            ),
            (
                HashMap::from([("NATURE".into(), "Nationale".into())]),
                HashMap::from([("Type".into(), "0x02".into())]),
            ),
        ]);
        let attrs = HashMap::from([("NATURE".into(), "Nationale".into())]);
        let result = evaluate_feature(&ruleset, &attrs).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().get("Type").unwrap(), "0x02");
    }

    #[test]
    fn test_evaluate_feature_first_match_wins_when_multiple_match() {
        let ruleset = make_ruleset(vec![
            (
                HashMap::from([("NATURE".into(), "*".into())]),
                HashMap::from([("Type".into(), "0x0a".into())]),
            ),
            (
                HashMap::from([("NATURE".into(), "Autoroute".into())]),
                HashMap::from([("Type".into(), "0x01".into())]),
            ),
        ]);
        let attrs = HashMap::from([("NATURE".into(), "Autoroute".into())]);
        let result = evaluate_feature(&ruleset, &attrs).unwrap();
        assert!(result.is_some());
        // First rule (wildcard) wins even though second rule also matches
        assert_eq!(result.unwrap().get("Type").unwrap(), "0x0a");
    }

    #[test]
    fn test_evaluate_feature_no_match_returns_none() {
        let ruleset = make_ruleset(vec![(
            HashMap::from([("NATURE".into(), "Autoroute".into())]),
            HashMap::from([("Type".into(), "0x01".into())]),
        )]);
        let attrs = HashMap::from([("NATURE".into(), "Sentier".into())]);
        let result = evaluate_feature(&ruleset, &attrs).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_evaluate_feature_invalid_type_returns_error() {
        let ruleset = make_ruleset(vec![(
            HashMap::from([("NATURE".into(), "Autoroute".into())]),
            HashMap::from([("Type".into(), "invalid".into())]),
        )]);
        let attrs = HashMap::from([("NATURE".into(), "Autoroute".into())]);
        let result = evaluate_feature(&ruleset, &attrs);
        assert!(result.is_err());
    }

    #[test]
    fn test_error_unsupported_version() {
        let yaml = r#"
version: 2
rulesets:
  - name: "Routes"
    source_layer: "TRONCON_DE_ROUTE"
    rules:
      - match:
          CL_ADMIN: "Autoroute"
        set:
          Type: "0x01"
"#;
        let rules: RulesFile = serde_yml::from_str(yaml).unwrap();
        let result = validate_rules(&rules);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Unsupported rules file version: 2"));
    }

    // ========================================================================
    // Story 9.3 - Task 2: find_ruleset tests
    // ========================================================================

    fn make_rules_file(layers: &[&str]) -> RulesFile {
        RulesFile {
            version: 1,
            rulesets: layers
                .iter()
                .map(|name| Ruleset {
                    name: Some(name.to_string()),
                    source_layer: name.to_string(),
                    rules: vec![],
                })
                .collect(),
        }
    }

    #[test]
    fn test_find_ruleset_found() {
        let rules = make_rules_file(&["TRONCON_DE_ROUTE", "BATIMENT"]);
        let result = find_ruleset(&rules, "TRONCON_DE_ROUTE");
        assert!(result.is_some());
        assert_eq!(result.unwrap().source_layer, "TRONCON_DE_ROUTE");
    }

    #[test]
    fn test_find_ruleset_not_found() {
        let rules = make_rules_file(&["TRONCON_DE_ROUTE", "BATIMENT"]);
        let result = find_ruleset(&rules, "UNKNOWN_LAYER");
        assert!(result.is_none());
    }

    #[test]
    fn test_find_ruleset_case_sensitive() {
        let rules = make_rules_file(&["TRONCON_DE_ROUTE"]);
        assert!(find_ruleset(&rules, "troncon_de_route").is_none());
        assert!(find_ruleset(&rules, "Troncon_De_Route").is_none());
        assert!(find_ruleset(&rules, "TRONCON_DE_ROUTE").is_some());
    }

    #[test]
    fn test_find_ruleset_multiple_returns_first() {
        let mut rules = make_rules_file(&["LAYER_A", "LAYER_A"]);
        rules.rulesets[0].name = Some("First".to_string());
        rules.rulesets[1].name = Some("Second".to_string());
        let result = find_ruleset(&rules, "LAYER_A");
        assert!(result.is_some());
        assert_eq!(result.unwrap().name, Some("First".to_string()));
    }

    #[test]
    fn test_find_ruleset_empty_rulesets() {
        let rules = make_rules_file(&[]);
        assert!(find_ruleset(&rules, "ANYTHING").is_none());
    }
}
