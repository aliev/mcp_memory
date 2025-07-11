use crate::graph::{Entity, KnowledgeGraph, Relation};
use anyhow::Result;
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};

/// Ranking algorithm used by the search engine
#[derive(Debug, Clone)]
pub struct SearchRanker {
    /// Weight for matching the entity name
    pub name_weight: f32,
    /// Weight for matching the entity type
    pub type_weight: f32,
    /// Weight for matching observations
    pub observation_weight: f32,
    /// Weight for the number of observations
    pub observation_count_weight: f32,
    /// Weight for connectivity (number of relations)
    pub connectivity_weight: f32,
}

impl Default for SearchRanker {
    fn default() -> Self {
        Self {
            name_weight: 2.0,
            type_weight: 1.5,
            observation_weight: 1.0,
            observation_count_weight: 0.5,
            connectivity_weight: 0.3,
        }
    }
}

impl SearchRanker {
    /// Calculate the relevance of an entity for a text query
    pub fn calculate_text_relevance(
        &self,
        entity: &Entity,
        query: &str,
        relations: &[Relation],
    ) -> f32 {
        let query_lower = query.to_lowercase();
        let mut score = 0.0;

        // Name match (prefer exact match)
        if entity.name.to_lowercase() == query_lower {
            score += self.name_weight * 2.0;
        } else if entity.name.to_lowercase().contains(&query_lower) {
            score += self.name_weight;
        }

        // Type match
        if entity.entity_type.to_lowercase().contains(&query_lower) {
            score += self.type_weight;
        }

        // Observation matches
        let observation_matches = entity
            .observations
            .iter()
            .filter(|obs| obs.to_lowercase().contains(&query_lower))
            .count();

        if observation_matches > 0 {
            score += self.observation_weight * observation_matches as f32;
        }

        // Bonus for the number of observations
        score += self.observation_count_weight * (entity.observations.len() as f32).ln_1p();

        // Bonus for connectivity
        let connection_count = relations
            .iter()
            .filter(|r| r.from == entity.name || r.to == entity.name)
            .count();
        score += self.connectivity_weight * (connection_count as f32).ln_1p();

        score
    }
}

/// Simple search engine with ranking
pub struct SearchEngine {
    ranker: SearchRanker,
}

impl SearchEngine {
    pub fn new() -> Self {
        Self {
            ranker: SearchRanker::default(),
        }
    }

    /// Text search with ranking and caching
    pub async fn enhanced_text_search(
        &self,
        graph: &KnowledgeGraph,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Vec<Entity>> {
        let _start_time = std::time::Instant::now();

        let _query_lower = query.to_lowercase();
        let entities: Vec<_> = graph.entities.values().collect();

        // Compute relevance scores in parallel
        let mut scored_entities: Vec<_> = entities
            .par_iter()
            .filter_map(|entity| {
                let relevance =
                    self.ranker
                        .calculate_text_relevance(entity, query, &graph.relations);
                if relevance > 0.0 {
                    Some(((*entity).clone(), relevance))
                } else {
                    None
                }
            })
            .collect();

        // Sort by relevance
        scored_entities.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let limit = limit.unwrap_or(10);
        let results: Vec<Entity> = scored_entities
            .into_iter()
            .take(limit)
            .map(|(entity, _)| entity)
            .collect();

        Ok(results)
    }

    /// Filter relations based on the found entities
    pub fn filter_relations_smart(
        &self,
        relations: &[Relation],
        entity_names: &HashSet<String>,
        show_all_relations: bool,
        include_related_entities: bool,
    ) -> Vec<Relation> {
        if show_all_relations {
            // Include all relations where at least one side was found
            relations
                .iter()
                .filter(|r| entity_names.contains(&r.from) || entity_names.contains(&r.to))
                .cloned()
                .collect()
        } else if include_related_entities {
            // Include relations between found entities and highly connected neighbors
            let mut related_entities = entity_names.clone();

            // Track entities linked to many of the found entities
            let mut entity_connections: HashMap<String, usize> = HashMap::new();
            for relation in relations {
                if entity_names.contains(&relation.from) {
                    *entity_connections.entry(relation.to.clone()).or_insert(0) += 1;
                }
                if entity_names.contains(&relation.to) {
                    *entity_connections.entry(relation.from.clone()).or_insert(0) += 1;
                }
            }

            // Add highly connected entities (>=2 links)
            for (entity, connections) in entity_connections {
                if connections >= 2 {
                    related_entities.insert(entity);
                }
            }

            relations
                .iter()
                .filter(|r| related_entities.contains(&r.from) && related_entities.contains(&r.to))
                .cloned()
                .collect()
        } else {
            // Only relations between the found entities
            relations
                .iter()
                .filter(|r| entity_names.contains(&r.from) && entity_names.contains(&r.to))
                .cloned()
                .collect()
        }
    }
}
