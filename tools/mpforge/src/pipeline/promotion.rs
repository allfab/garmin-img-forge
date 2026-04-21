//! Tech-spec overview wide-zoom : moteur de promotion feature → palier overview.
//!
//! Appliqué APRÈS le moteur de règles (`garmin-rules.yaml`) et AVANT
//! `generalize_features_with_profiles` : chaque feature matchée voit son
//! attribut `EndLevel` remonté jusqu'à `promote_to`, de sorte que
//! `fill_level_gaps` comble ensuite jusqu'au palier overview cible.
//!
//! Première règle qui matche gagne (cohérent avec `garmin-rules.yaml`). La
//! promotion n'abaisse jamais un `EndLevel` déjà plus élevé (non-régression).

use crate::config::{OverviewLevels, PromotionRule};
use crate::pipeline::reader::Feature;
use std::collections::HashMap;

/// Clé de l'attribut `EndLevel` sur `Feature.attributes`. Aligné avec le
/// mapping `garmin-rules.yaml` qui écrit cette clé via `set: { EndLevel: ... }`.
const END_LEVEL_KEY: &str = "EndLevel";

/// Évalue les règles de promotion pour une feature et retourne le `promote_to`
/// de la première règle matchée (`None` si aucune règle ne matche).
pub fn resolve_promotion(
    feature: &Feature,
    layer: &str,
    rules: &OverviewLevels,
) -> Option<u8> {
    let layer_rules = rules.promotion.get(layer)?;
    // Dispatch sur les attributs BDTOPO source (pré-règles).
    // Fallback sur attributes si source_attributes absent (configs sans overview_levels).
    let attrs = feature.source_attributes.as_ref().unwrap_or(&feature.attributes);
    for rule in layer_rules {
        if matches_rule_attrs(rule, attrs) {
            return Some(rule.promote_to);
        }
    }
    None
}

/// Applique la promotion à une feature : si une règle matche, réécrit l'attribut
/// `EndLevel` avec `max(current, promote_to)` (non-régression).
pub fn apply_promotion(feature: &mut Feature, layer: &str, rules: &OverviewLevels) {
    let Some(promote_to) = resolve_promotion(feature, layer, rules) else {
        return;
    };
    let current = feature
        .attributes
        .get(END_LEVEL_KEY)
        .and_then(|s| s.parse::<u8>().ok())
        .unwrap_or(0);
    let effective = current.max(promote_to);
    feature
        .attributes
        .insert(END_LEVEL_KEY.to_string(), effective.to_string());
}

/// AND sur tous les champs de `match`. Pour chaque champ, au moins une valeur
/// de la liste doit matcher (ou ne pas matcher, si préfixe `!`).
fn matches_rule_attrs(rule: &PromotionRule, attrs: &HashMap<String, String>) -> bool {
    for (field, values) in &rule.match_ {
        if !field_matches(values, attrs.get(field).map(|s| s.as_str())) {
            return false;
        }
    }
    true
}

/// Matching d'un champ : chaque valeur est soit positive (match égalité), soit
/// négative (préfixe `!` → match si différent). La liste est évaluée en OR
/// pour les positives ; une règle négative fait échouer immédiatement si la
/// valeur interdite est présente.
fn field_matches(values: &[String], observed: Option<&str>) -> bool {
    let observed = observed.unwrap_or("");
    let mut has_positive = false;
    let mut positive_hit = false;
    for v in values {
        if let Some(forbidden) = v.strip_prefix('!') {
            if observed == forbidden {
                return false;
            }
        } else {
            has_positive = true;
            if observed == v {
                positive_hit = true;
            }
        }
    }
    if has_positive {
        positive_hit
    } else {
        // Uniquement des négations → match par défaut (aucun interdit matché).
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::reader::{Feature, GeometryType};
    use std::collections::{BTreeMap, HashMap};

    fn make_feature(layer: &str, attrs: &[(&str, &str)]) -> Feature {
        let mut attributes = HashMap::new();
        for (k, v) in attrs {
            attributes.insert((*k).to_string(), (*v).to_string());
        }
        Feature {
            geometry_type: GeometryType::LineString,
            geometry: vec![(0.0, 0.0), (1.0, 0.0)],
            additional_geometries: Default::default(),
            attributes,
            source_attributes: None,
            source_layer: Some(layer.to_string()),
        }
    }

    fn ov_with_rules(layer: &str, rules: Vec<PromotionRule>) -> OverviewLevels {
        let mut promotion = BTreeMap::new();
        promotion.insert(layer.to_string(), rules);
        OverviewLevels {
            header_extension: vec![14, 12, 10],
            promotion,
        }
    }

    fn rule(pairs: &[(&str, &[&str])], promote_to: u8) -> PromotionRule {
        let mut match_ = BTreeMap::new();
        for (k, vs) in pairs {
            match_.insert(
                (*k).to_string(),
                vs.iter().map(|s| (*s).to_string()).collect(),
            );
        }
        PromotionRule { match_, promote_to }
    }

    #[test]
    fn test_happy_path_single_rule() {
        let ov = ov_with_rules(
            "TRONCON_DE_ROUTE",
            vec![rule(&[("CL_ADMIN", &["Autoroute"])], 9)],
        );
        let mut f = make_feature("TRONCON_DE_ROUTE", &[("CL_ADMIN", "Autoroute")]);
        apply_promotion(&mut f, "TRONCON_DE_ROUTE", &ov);
        assert_eq!(f.attributes.get("EndLevel").map(|s| s.as_str()), Some("9"));
    }

    #[test]
    fn test_first_match_wins() {
        // AC6 : deux règles matchent, la première doit l'emporter (9, pas 8).
        let ov = ov_with_rules(
            "TRONCON_DE_ROUTE",
            vec![
                rule(&[("CL_ADMIN", &["Autoroute"])], 9),
                rule(&[("CL_ADMIN", &["!Communale"])], 8),
            ],
        );
        let mut f = make_feature("TRONCON_DE_ROUTE", &[("CL_ADMIN", "Autoroute")]);
        apply_promotion(&mut f, "TRONCON_DE_ROUTE", &ov);
        assert_eq!(f.attributes["EndLevel"], "9");
    }

    #[test]
    fn test_no_match_leaves_endlevel_untouched() {
        let ov = ov_with_rules(
            "TRONCON_DE_ROUTE",
            vec![rule(&[("CL_ADMIN", &["Autoroute"])], 9)],
        );
        let mut f = make_feature(
            "TRONCON_DE_ROUTE",
            &[("CL_ADMIN", "Communale"), ("EndLevel", "3")],
        );
        apply_promotion(&mut f, "TRONCON_DE_ROUTE", &ov);
        assert_eq!(f.attributes["EndLevel"], "3");
    }

    #[test]
    fn test_no_regression_when_current_is_higher() {
        // AC8 : EndLevel=9 préexistant, promotion demande 7 → reste 9.
        let ov = ov_with_rules(
            "TRONCON_DE_ROUTE",
            vec![rule(&[("CL_ADMIN", &["Nationale"])], 7)],
        );
        let mut f = make_feature(
            "TRONCON_DE_ROUTE",
            &[("CL_ADMIN", "Nationale"), ("EndLevel", "9")],
        );
        apply_promotion(&mut f, "TRONCON_DE_ROUTE", &ov);
        assert_eq!(f.attributes["EndLevel"], "9");
    }

    #[test]
    fn test_negation_prefix() {
        // `!Communale` : match sur toute valeur ≠ Communale.
        let ov = ov_with_rules(
            "TRONCON_DE_ROUTE",
            vec![rule(&[("CL_ADMIN", &["!Communale"])], 8)],
        );
        let mut matching =
            make_feature("TRONCON_DE_ROUTE", &[("CL_ADMIN", "Départementale")]);
        apply_promotion(&mut matching, "TRONCON_DE_ROUTE", &ov);
        assert_eq!(matching.attributes["EndLevel"], "8");

        let mut excluded = make_feature("TRONCON_DE_ROUTE", &[("CL_ADMIN", "Communale")]);
        apply_promotion(&mut excluded, "TRONCON_DE_ROUTE", &ov);
        assert!(!excluded.attributes.contains_key("EndLevel"));
    }

    #[test]
    fn test_missing_endlevel_treated_as_zero() {
        let ov = ov_with_rules(
            "TRONCON_DE_ROUTE",
            vec![rule(&[("CL_ADMIN", &["Autoroute"])], 9)],
        );
        let mut f = make_feature("TRONCON_DE_ROUTE", &[("CL_ADMIN", "Autoroute")]);
        // pas d'EndLevel initial
        apply_promotion(&mut f, "TRONCON_DE_ROUTE", &ov);
        assert_eq!(f.attributes["EndLevel"], "9");
    }

    #[test]
    fn test_and_semantics_across_fields() {
        // Deux champs à matcher simultanément.
        let ov = ov_with_rules(
            "TRONCON_DE_ROUTE",
            vec![rule(
                &[
                    ("CL_ADMIN", &["Autoroute"]),
                    ("NATURE", &["Bretelle"]),
                ],
                9,
            )],
        );
        let mut both = make_feature(
            "TRONCON_DE_ROUTE",
            &[("CL_ADMIN", "Autoroute"), ("NATURE", "Bretelle")],
        );
        apply_promotion(&mut both, "TRONCON_DE_ROUTE", &ov);
        assert_eq!(both.attributes["EndLevel"], "9");

        let mut one = make_feature(
            "TRONCON_DE_ROUTE",
            &[("CL_ADMIN", "Autoroute"), ("NATURE", "Route à 1 chaussée")],
        );
        apply_promotion(&mut one, "TRONCON_DE_ROUTE", &ov);
        assert!(!one.attributes.contains_key("EndLevel"));
    }

    #[test]
    fn test_unknown_layer_noop() {
        let ov = ov_with_rules(
            "TRONCON_DE_ROUTE",
            vec![rule(&[("CL_ADMIN", &["Autoroute"])], 9)],
        );
        let mut f = make_feature("BATIMENT", &[("CL_ADMIN", "Autoroute")]);
        apply_promotion(&mut f, "BATIMENT", &ov);
        assert!(!f.attributes.contains_key("EndLevel"));
    }

    #[test]
    fn test_promotion_uses_source_attributes_when_attributes_stripped() {
        // Cas clé du fix source_attributes : CL_ADMIN absent de attributes
        // (post-règles) mais présent dans source_attributes (pré-règles).
        let ov = ov_with_rules(
            "TRONCON_DE_ROUTE",
            vec![rule(&[("CL_ADMIN", &["Autoroute"])], 9)],
        );
        let mut f = make_feature("TRONCON_DE_ROUTE", &[("Type", "0x01"), ("EndLevel", "4")]);
        f.source_attributes = Some({
            let mut m = HashMap::new();
            m.insert("CL_ADMIN".to_string(), "Autoroute".to_string());
            m
        });
        apply_promotion(&mut f, "TRONCON_DE_ROUTE", &ov);
        assert_eq!(
            f.attributes["EndLevel"], "9",
            "promotion doit monter EndLevel via source_attributes même si CL_ADMIN absent de attributes"
        );
    }

    #[test]
    fn test_no_promotion_when_source_attributes_absent_and_attributes_stripped() {
        // Sans source_attributes ni CL_ADMIN dans attributes → pas de promotion.
        let ov = ov_with_rules(
            "TRONCON_DE_ROUTE",
            vec![rule(&[("CL_ADMIN", &["Autoroute"])], 9)],
        );
        let mut f = make_feature("TRONCON_DE_ROUTE", &[("Type", "0x01"), ("EndLevel", "4")]);
        // source_attributes absent → fallback sur attributes → pas de CL_ADMIN → pas de match
        apply_promotion(&mut f, "TRONCON_DE_ROUTE", &ov);
        assert_eq!(
            f.attributes["EndLevel"], "4",
            "pas de promotion quand ni source_attributes ni attributes ne portent le champ de dispatch"
        );
    }
}
