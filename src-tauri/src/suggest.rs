use crate::data_dragon::ChampionMeta;
use crate::pool::PoolEntry;
use serde::Serialize;
use std::collections::{HashMap, HashSet};

const POOL_WEIGHT: f32 = 0.65;
const COMP_FIT_WEIGHT: f32 = 0.35;
const MAX_SUGGESTIONS: usize = 5;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedPick {
    pub champion_id: u32,
    pub champion_name: String,
    pub pool_score: f32,
    pub comp_fit_score: f32,
    pub total_score: f32,
    pub reasons: Vec<String>,
}

/// Sugestão heurística: combina o score do seu pool (maestria + win rate recente +
/// nota) com um bônus por preencher lacunas óbvias da composição aliada (falta de
/// tank, time todo do mesmo tipo de dano). Isso NÃO é dado de contra-pick real — é
/// só uma aproximação a partir das tags de classe do próprio Data Dragon.
pub fn suggest(
    pool: &[PoolEntry],
    names: &HashMap<u32, ChampionMeta>,
    unavailable_champion_ids: &HashSet<u32>,
    ally_champion_ids: &[u32],
) -> Vec<SuggestedPick> {
    let ally_tags: Vec<&str> = ally_champion_ids
        .iter()
        .filter(|id| **id != 0)
        .filter_map(|id| names.get(id))
        .flat_map(|meta| meta.tags.iter().map(String::as_str))
        .collect();

    let missing_tank = !ally_tags.iter().copied().any(|t| t == "Tank");
    let physical_count = ally_tags
        .iter()
        .copied()
        .filter(|t| matches!(*t, "Marksman" | "Fighter" | "Assassin"))
        .count();
    let magic_count = ally_tags.iter().copied().filter(|t| *t == "Mage").count();

    let mut suggestions: Vec<SuggestedPick> = pool
        .iter()
        .filter(|entry| !unavailable_champion_ids.contains(&entry.champion_id))
        .filter_map(|entry| {
            let meta = names.get(&entry.champion_id)?;
            let mut reasons = Vec::new();
            let mut comp_fit: f32 = 0.5;

            if missing_tank && meta.tags.iter().any(|t| t == "Tank") {
                comp_fit += 0.3;
                reasons.push("seu time ainda não tem tank".to_string());
            }

            let is_magic = meta.tags.iter().any(|t| t == "Mage");
            let is_physical = meta
                .tags
                .iter()
                .any(|t| matches!(t.as_str(), "Marksman" | "Fighter" | "Assassin"));

            if magic_count == 0 && physical_count >= 2 && is_magic {
                comp_fit += 0.2;
                reasons.push("time está concentrado em dano físico".to_string());
            } else if physical_count == 0 && magic_count >= 2 && is_physical {
                comp_fit += 0.2;
                reasons.push("time está concentrado em dano mágico".to_string());
            }

            comp_fit = comp_fit.min(1.0);

            if reasons.is_empty() {
                reasons.push("bem avaliado no seu pool (maestria/win rate recente)".to_string());
            }

            let total_score = entry.score * POOL_WEIGHT + comp_fit * COMP_FIT_WEIGHT;

            Some(SuggestedPick {
                champion_id: entry.champion_id,
                champion_name: entry.champion_name.clone(),
                pool_score: entry.score,
                comp_fit_score: comp_fit,
                total_score,
                reasons,
            })
        })
        .collect();

    suggestions.sort_by(|a, b| b.total_score.partial_cmp(&a.total_score).unwrap_or(std::cmp::Ordering::Equal));
    suggestions.truncate(MAX_SUGGESTIONS);
    suggestions
}
