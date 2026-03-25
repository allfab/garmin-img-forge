//! Rules file parsing and validation for attribute transformation.
//!
//! Story 9.1: Loads and validates a YAML rules file that defines
//! how source attributes are transformed for Polish Map export.

use anyhow::Context;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use tracing::info;

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
}
