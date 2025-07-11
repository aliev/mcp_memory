use anyhow::{Context, Result};
use rmcp::schemars;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::search::SearchEngine;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Entity {
    #[schemars(description = "The name of the entity")]
    pub name: String,
    #[serde(rename = "entityType")]
    #[schemars(description = "The type of the entity")]
    pub entity_type: String,
    #[schemars(description = "An array of observation contents associated with the entity")]
    pub observations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
pub struct Relation {
    #[schemars(description = "The name of the entity where the relation starts")]
    pub from: String,
    #[schemars(description = "The name of the entity where the relation ends")]
    pub to: String,
    #[serde(rename = "relationType")]
    #[schemars(description = "The type of the relation")]
    pub relation_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeGraph {
    pub entities: std::collections::HashMap<String, Entity>,
    pub relations: Vec<Relation>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
enum GraphItem {
    Entity(Entity),
    Relation(Relation),
}

pub struct KnowledgeGraphManager {
    memory_file_path: PathBuf,
    search_engine: Arc<SearchEngine>,
}

impl KnowledgeGraphManager {
    pub fn new() -> Self {
        let current_exe = env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
        let current_dir = current_exe.parent().unwrap_or_else(|| Path::new("."));
        let default_memory_path = current_dir.join("memory.jsonl");

        let memory_file_path = if let Ok(path_env) = env::var("MEMORY_FILE_PATH") {
            let path = PathBuf::from(path_env);
            if path.is_absolute() {
                path
            } else {
                current_dir.join(path)
            }
        } else {
            default_memory_path
        };

        let search_engine = Arc::new(SearchEngine::new());

        Self {
            memory_file_path,
            search_engine,
        }
    }

    pub fn with_path<P: AsRef<Path>>(path: P) -> Self {
        let memory_file_path = path.as_ref().to_path_buf();

        let search_engine = Arc::new(SearchEngine::new());

        Self {
            memory_file_path,
            search_engine,
        }
    }

    async fn load_graph(&self) -> Result<Box<KnowledgeGraph>> {
        let _start_time = Instant::now();

        match fs::read_to_string(&self.memory_file_path).await {
            Ok(data) => {
                let mut entities = Vec::new();
                let mut relations = Vec::new();

                for line in data.lines() {
                    if line.trim().is_empty() {
                        continue;
                    }

                    let item: GraphItem = serde_json::from_str(line)
                        .with_context(|| format!("Failed to parse JSON line: {line}"))?;
                    match item {
                        GraphItem::Entity(entity) => entities.push(entity),
                        GraphItem::Relation(relation) => relations.push(relation),
                    }
                }

                let graph = KnowledgeGraph {
                    entities: entities.into_iter().map(|e| (e.name.clone(), e)).collect(),
                    relations,
                };

                Ok(Box::new(graph))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Box::new(KnowledgeGraph {
                entities: HashMap::new(),
                relations: Vec::new(),
            })),
            Err(e) => Err(e).with_context(|| {
                format!(
                    "Failed to read graph from {}",
                    self.memory_file_path.display()
                )
            }),
        }
    }

    async fn save_graph(&self, graph: &KnowledgeGraph) -> Result<()> {
        let mut data = String::new();

        for entity in graph.entities.values() {
            let item = GraphItem::Entity(entity.clone());
            let line = serde_json::to_string(&item)
                .with_context(|| format!("Failed to serialize entity {}", entity.name))?;
            data.push_str(&line);
            data.push('\n');
        }

        for relation in &graph.relations {
            let item = GraphItem::Relation(relation.clone());
            let line = serde_json::to_string(&item).with_context(|| {
                format!(
                    "Failed to serialize relation {} -> {}",
                    relation.from, relation.to
                )
            })?;
            data.push_str(&line);
            data.push('\n');
        }

        let mut file = fs::File::create(&self.memory_file_path)
            .await
            .with_context(|| {
                format!("Failed to create file {}", self.memory_file_path.display())
            })?;
        file.write_all(data.as_bytes())
            .await
            .with_context(|| "Failed to write graph to file")?;

        Ok(())
    }

    pub async fn create_entities(&self, entities: Vec<Entity>) -> Result<Vec<Entity>> {
        let mut graph = self.load_graph().await?;
        let existing_names: HashSet<_> = graph.entities.values().map(|e| &e.name).collect();

        let new_entities: Vec<Entity> = entities
            .into_iter()
            .filter(|e| !existing_names.contains(&e.name))
            .collect();

        for entity in new_entities.iter() {
            graph.entities.insert(entity.name.clone(), entity.clone());
        }
        self.save_graph(&graph).await?;

        Ok(new_entities)
    }

    pub async fn create_relations(&self, relations: Vec<Relation>) -> Result<Vec<Relation>> {
        let mut graph = self.load_graph().await?;
        let existing_relations: HashSet<_> = graph
            .relations
            .iter()
            .map(|r| (&r.from, &r.to, &r.relation_type))
            .collect();

        let new_relations: Vec<Relation> = relations
            .into_iter()
            .filter(|r| !existing_relations.contains(&(&r.from, &r.to, &r.relation_type)))
            .collect();

        graph.relations.extend(new_relations.clone());
        self.save_graph(&graph).await?;

        Ok(new_relations)
    }

    pub async fn add_observations(
        &self,
        observations: Vec<(String, Vec<String>)>,
    ) -> Result<Vec<(String, Vec<String>)>> {
        let mut graph = self.load_graph().await?;
        let mut results = Vec::new();

        for (entity_name, contents) in observations {
            let entity = graph
                .entities
                .get_mut(&entity_name)
                .with_context(|| format!("Entity with name '{entity_name}' not found"))?;

            let existing_observations: HashSet<_> = entity.observations.iter().collect();
            let new_observations: Vec<String> = contents
                .into_iter()
                .filter(|content| !existing_observations.contains(content))
                .collect();

            entity.observations.extend(new_observations.clone());
            results.push((entity_name, new_observations));
        }

        self.save_graph(&graph).await?;
        Ok(results)
    }

    pub async fn delete_entities(&self, entity_names: Vec<String>) -> Result<()> {
        let mut graph = self.load_graph().await?;
        let names_set: HashSet<_> = entity_names.iter().collect();

        graph.entities.retain(|_, e| !names_set.contains(&e.name));
        graph
            .relations
            .retain(|r| !names_set.contains(&r.from) && !names_set.contains(&r.to));

        self.save_graph(&graph).await?;
        Ok(())
    }

    pub async fn delete_observations(&self, deletions: Vec<(String, Vec<String>)>) -> Result<()> {
        let mut graph = self.load_graph().await?;

        for (entity_name, observations_to_delete) in deletions {
            if let Some(entity) = graph.entities.get_mut(&entity_name) {
                let delete_set: HashSet<_> = observations_to_delete.iter().collect();
                entity.observations.retain(|o| !delete_set.contains(&o));
            }
        }

        self.save_graph(&graph).await?;
        Ok(())
    }

    pub async fn delete_relations(&self, relations: Vec<Relation>) -> Result<()> {
        let mut graph = self.load_graph().await?;
        let relations_to_delete: HashSet<_> = relations
            .iter()
            .map(|r| (&r.from, &r.to, &r.relation_type))
            .collect();

        graph
            .relations
            .retain(|r| !relations_to_delete.contains(&(&r.from, &r.to, &r.relation_type)));

        self.save_graph(&graph).await?;
        Ok(())
    }

    pub async fn read_graph(&self) -> Result<Box<KnowledgeGraph>> {
        self.load_graph().await
    }

    pub async fn open_nodes(&self, names: Vec<String>) -> Result<Box<KnowledgeGraph>> {
        let graph = self.load_graph().await?;
        let names_set: HashSet<_> = names.iter().collect();

        let filtered_entities: Vec<Entity> = graph
            .entities
            .values()
            .filter(|&e| names_set.contains(&e.name))
            .cloned()
            .collect();

        let filtered_entity_names: HashSet<_> = filtered_entities.iter().map(|e| &e.name).collect();

        let filtered_relations: Vec<Relation> = graph
            .relations
            .into_iter()
            .filter(|r| {
                filtered_entity_names.contains(&r.from) && filtered_entity_names.contains(&r.to)
            })
            .collect();

        Ok(Box::new(KnowledgeGraph {
            entities: filtered_entities
                .into_iter()
                .map(|e| (e.name.clone(), e))
                .collect(),
            relations: filtered_relations,
        }))
    }

    pub async fn get_stats(&self) -> Result<(usize, usize)> {
        let graph = self.load_graph().await?;
        Ok((graph.entities.len(), graph.relations.len()))
    }

    pub async fn search_nodes(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> Result<Box<KnowledgeGraph>> {
        let graph = self.load_graph().await?;

        let entities = self
            .search_engine
            .enhanced_text_search(&graph, query, limit)
            .await?;

        let filtered_entity_names: HashSet<String> =
            entities.iter().map(|e| e.name.clone()).collect();

        let filtered_relations = self.search_engine.filter_relations_smart(
            &graph.relations,
            &filtered_entity_names,
            false,
            false,
        );

        Ok(Box::new(KnowledgeGraph {
            entities: entities.into_iter().map(|e| (e.name.clone(), e)).collect(),
            relations: filtered_relations,
        }))
    }
}

impl Default for KnowledgeGraphManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_create_and_read_entities() -> Result<()> {
        let temp_dir = tempdir()?;
        let temp_path = temp_dir.path().join("test_memory.jsonl");
        let manager = KnowledgeGraphManager::with_path(&temp_path);

        let entities = vec![Entity {
            name: "Alice".to_string(),
            entity_type: "Person".to_string(),
            observations: vec!["Likes coffee".to_string()],
        }];

        let created = manager.create_entities(entities.clone()).await?;
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].name, "Alice");

        let graph = manager.read_graph().await?;
        assert_eq!(graph.entities.len(), 1);
        let alice = graph.entities.get("Alice").unwrap();
        assert_eq!(alice.name, "Alice");

        Ok(())
    }

    #[tokio::test]
    async fn test_boxed_manager() -> Result<()> {
        let temp_dir = tempdir()?;
        let temp_path = temp_dir.path().join("test_memory.jsonl");
        let manager = Box::new(KnowledgeGraphManager::with_path(&temp_path));

        let entities = vec![Entity {
            name: "BoxedEntity".to_string(),
            entity_type: "Test".to_string(),
            observations: vec!["Created with Box".to_string()],
        }];

        let created = manager.create_entities(entities).await?;
        assert_eq!(created.len(), 1);
        assert_eq!(created[0].name, "BoxedEntity");

        Ok(())
    }

    #[tokio::test]
    async fn test_large_graph_in_heap() -> Result<()> {
        let temp_dir = tempdir()?;
        let temp_path = temp_dir.path().join("test_large_memory.jsonl");
        let manager = KnowledgeGraphManager::with_path(&temp_path);

        let mut large_entities = Vec::new();
        for i in 0..10000 {
            large_entities.push(Entity {
                name: format!("LargeEntity_{i}"),
                entity_type: "TestLarge".to_string(),
                observations: vec![
                    format!("Observation 1 for {}", i),
                    format!("Observation 2 for {}", i),
                    format!("Large data set with ID {}", i),
                ],
            });
        }

        let created = manager.create_entities(large_entities).await?;
        assert_eq!(created.len(), 10000);

        let (entity_count, relation_count) = manager.get_stats().await?;
        assert_eq!(entity_count, 10000);
        assert_eq!(relation_count, 0);

        let search_result = manager.search_nodes("LargeEntity_9999", None).await?;
        // Enhanced search may return multiple similar entities
        assert!(
            !search_result.entities.is_empty(),
            "Should find at least LargeEntity_9999"
        );
        assert!(
            search_result.entities.contains_key("LargeEntity_9999"),
            "Should find LargeEntity_9999"
        );
        let large_entity = search_result.entities.get("LargeEntity_9999").unwrap();
        assert_eq!(large_entity.name, "LargeEntity_9999");

        let full_graph = manager.read_graph().await?;
        assert_eq!(full_graph.entities.len(), 10000);

        println!(
            "Successfully handled {} entities in heap",
            full_graph.entities.len()
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_arc_manager() -> Result<()> {
        let temp_dir = tempdir()?;
        let temp_path = temp_dir.path().join("test_memory.jsonl");
        let manager = std::sync::Arc::new(KnowledgeGraphManager::with_path(&temp_path));
        let manager_clone = manager.clone();

        assert_eq!(std::sync::Arc::strong_count(&manager), 2);

        let entities = vec![Entity {
            name: "ArcEntity".to_string(),
            entity_type: "Test".to_string(),
            observations: vec!["Created with Arc".to_string()],
        }];

        let created = manager_clone.create_entities(entities).await?;
        assert_eq!(created.len(), 1);

        let graph = manager.read_graph().await?; // Box<KnowledgeGraph>
        assert_eq!(graph.entities.len(), 1);
        let arc_entity = graph.entities.get("ArcEntity").unwrap();
        assert_eq!(arc_entity.name, "ArcEntity");

        Ok(())
    }

    #[tokio::test]
    async fn test_create_relations() -> Result<()> {
        let temp_dir = tempdir()?;
        let temp_path = temp_dir.path().join("test_memory.jsonl");
        let manager = KnowledgeGraphManager::with_path(&temp_path);

        // Ensure we start with a clean graph
        let initial_graph = manager.read_graph().await?;
        assert_eq!(initial_graph.relations.len(), 0);

        let entities = vec![
            Entity {
                name: "Alice".to_string(),
                entity_type: "Person".to_string(),
                observations: vec![],
            },
            Entity {
                name: "Bob".to_string(),
                entity_type: "Person".to_string(),
                observations: vec![],
            },
        ];

        manager.create_entities(entities).await?;

        // Verify entities were created
        let graph_after_entities = manager.read_graph().await?;
        assert_eq!(graph_after_entities.entities.len(), 2);
        assert_eq!(graph_after_entities.relations.len(), 0);

        let relations = vec![Relation {
            from: "Alice".to_string(),
            to: "Bob".to_string(),
            relation_type: "knows".to_string(),
        }];

        let created_relations = manager.create_relations(relations.clone()).await?;

        // Debug information for CI failures
        if created_relations.len() != 1 {
            let current_graph = manager.read_graph().await?;
            panic!(
                "Expected 1 created relation, got {}. Current graph has {} relations: {:?}. Trying to create: {:?}",
                created_relations.len(),
                current_graph.relations.len(),
                current_graph.relations,
                relations
            );
        }

        assert_eq!(created_relations.len(), 1);

        let graph = manager.read_graph().await?;
        assert_eq!(graph.relations.len(), 1);
        assert_eq!(graph.relations[0].from, "Alice");
        assert_eq!(graph.relations[0].to, "Bob");

        Ok(())
    }

    #[tokio::test]
    async fn test_search_nodes() -> Result<()> {
        let temp_dir = tempdir()?;
        let temp_path = temp_dir.path().join("test_memory.jsonl");
        let manager = KnowledgeGraphManager::with_path(&temp_path);

        let entities = vec![
            Entity {
                name: "Alice".to_string(),
                entity_type: "Person".to_string(),
                observations: vec!["Likes coffee".to_string()],
            },
            Entity {
                name: "Bob".to_string(),
                entity_type: "Person".to_string(),
                observations: vec!["Likes tea".to_string()],
            },
        ];

        manager.create_entities(entities).await?;

        let search_result = manager.search_nodes("coffee", None).await?;
        // The enhanced search may return more entities based on relevance scores
        assert!(
            !search_result.entities.is_empty(),
            "Should find at least Alice"
        );
        assert!(
            search_result.entities.contains_key("Alice"),
            "Should find Alice who likes coffee"
        );
        let alice = search_result.entities.get("Alice").unwrap();
        assert_eq!(alice.name, "Alice");

        Ok(())
    }

    #[tokio::test]
    async fn test_entity_not_found_error() {
        let temp_dir = tempdir().unwrap();
        let temp_path = temp_dir.path().join("test_memory.jsonl");
        let manager = KnowledgeGraphManager::with_path(&temp_path);

        let result = manager
            .add_observations(vec![(
                "NonExistent".to_string(),
                vec!["Some observation".to_string()],
            )])
            .await;

        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("NonExistent"));
        assert!(error_message.contains("not found"));
    }

    #[tokio::test]
    async fn test_stats_and_entity_exists() -> Result<()> {
        let temp_dir = tempdir()?;
        let temp_path = temp_dir.path().join("test_memory.jsonl");
        let manager = KnowledgeGraphManager::with_path(&temp_path);

        let (initial_entities, initial_relations) = manager.get_stats().await?;
        assert_eq!(initial_entities, 0);
        assert_eq!(initial_relations, 0);

        let entities = vec![
            Entity {
                name: "TestEntity1".to_string(),
                entity_type: "Test".to_string(),
                observations: vec![],
            },
            Entity {
                name: "TestEntity2".to_string(),
                entity_type: "Test".to_string(),
                observations: vec![],
            },
        ];

        manager.create_entities(entities).await?;

        let (updated_entities, updated_relations) = manager.get_stats().await?;
        assert_eq!(updated_entities, 2);
        assert_eq!(updated_relations, 0);

        Ok(())
    }
}
