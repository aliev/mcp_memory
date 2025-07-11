use std::future::Future;
use std::sync::Arc;

use rmcp::{
    handler::server::{router::tool::ToolRouter, tool::Parameters},
    model::{CallToolResult, Content, ErrorData as McpError, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router, ServerHandler,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::graph::{Entity, KnowledgeGraph, KnowledgeGraphManager, Relation};

#[async_trait::async_trait]
pub trait GraphService: Send + Sync + 'static {
    async fn create_entities(&self, entities: Vec<Entity>) -> anyhow::Result<Vec<Entity>>;
    async fn create_relations(&self, relations: Vec<Relation>) -> anyhow::Result<Vec<Relation>>;
    async fn search_nodes(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> anyhow::Result<Box<KnowledgeGraph>>;
    async fn get_stats(&self) -> anyhow::Result<(usize, usize)>;
    async fn read_graph(&self) -> anyhow::Result<Box<KnowledgeGraph>>;

    async fn add_observations(
        &self,
        observations: Vec<(String, Vec<String>)>,
    ) -> anyhow::Result<Vec<(String, Vec<String>)>>;
    async fn delete_entities(&self, entity_names: Vec<String>) -> anyhow::Result<()>;
    async fn delete_observations(
        &self,
        deletions: Vec<(String, Vec<String>)>,
    ) -> anyhow::Result<()>;
    async fn delete_relations(&self, relations: Vec<Relation>) -> anyhow::Result<()>;
    async fn open_nodes(&self, names: Vec<String>) -> anyhow::Result<Box<KnowledgeGraph>>;
}

#[derive(Clone)]
pub struct KnowledgeGraphService {
    manager: Arc<KnowledgeGraphManager>,
}

impl std::fmt::Debug for KnowledgeGraphService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KnowledgeGraphService")
            .field("manager", &"KnowledgeGraphManager")
            .finish()
    }
}

impl Default for KnowledgeGraphService {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeGraphService {
    pub fn new() -> Self {
        Self {
            manager: Arc::new(KnowledgeGraphManager::new()),
        }
    }

    pub fn with_path(path: impl AsRef<std::path::Path>) -> Self {
        Self {
            manager: Arc::new(KnowledgeGraphManager::with_path(path)),
        }
    }
}

#[async_trait::async_trait]
impl GraphService for KnowledgeGraphService {
    async fn create_entities(&self, entities: Vec<Entity>) -> anyhow::Result<Vec<Entity>> {
        self.manager.create_entities(entities).await
    }

    async fn create_relations(&self, relations: Vec<Relation>) -> anyhow::Result<Vec<Relation>> {
        self.manager.create_relations(relations).await
    }

    async fn search_nodes(
        &self,
        query: &str,
        limit: Option<usize>,
    ) -> anyhow::Result<Box<KnowledgeGraph>> {
        self.manager.search_nodes(query, limit).await
    }

    async fn get_stats(&self) -> anyhow::Result<(usize, usize)> {
        self.manager.get_stats().await
    }

    async fn read_graph(&self) -> anyhow::Result<Box<KnowledgeGraph>> {
        self.manager.read_graph().await
    }

    async fn add_observations(
        &self,
        observations: Vec<(String, Vec<String>)>,
    ) -> anyhow::Result<Vec<(String, Vec<String>)>> {
        self.manager.add_observations(observations).await
    }

    async fn delete_entities(&self, entity_names: Vec<String>) -> anyhow::Result<()> {
        self.manager.delete_entities(entity_names).await
    }

    async fn delete_observations(
        &self,
        deletions: Vec<(String, Vec<String>)>,
    ) -> anyhow::Result<()> {
        self.manager.delete_observations(deletions).await
    }

    async fn delete_relations(&self, relations: Vec<Relation>) -> anyhow::Result<()> {
        self.manager.delete_relations(relations).await
    }

    async fn open_nodes(&self, names: Vec<String>) -> anyhow::Result<Box<KnowledgeGraph>> {
        self.manager.open_nodes(names).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStats {
    pub entities: usize,
    pub relations: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateEntitiesRequest {
    pub entities: Vec<Entity>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateRelationsRequest {
    pub relations: Vec<Relation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchNodesRequest {
    #[schemars(
        description = "The search query to match against entity names, types, and observation content"
    )]
    pub query: String,
    #[schemars(description = "Maximum number of results to return")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AddObservationsRequest {
    pub observations: Vec<ObservationRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteEntitiesRequest {
    #[schemars(description = "An array of entity names to delete")]
    pub entity_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteObservationsRequest {
    pub deletions: Vec<ObservationDeletion>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteRelationsRequest {
    #[schemars(description = "An array of relations to delete")]
    pub relations: Vec<Relation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct OpenNodesRequest {
    #[schemars(description = "An array of entity names to retrieve")]
    pub names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ObservationRequest {
    #[serde(rename = "entityName")]
    pub entity_name: String,
    #[schemars(description = "An array of observation contents to add")]
    pub contents: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ObservationDeletion {
    #[serde(rename = "entityName")]
    pub entity_name: String,
    #[schemars(description = "An array of observations to delete")]
    pub observations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GraphServiceHandler<GS: GraphService> {
    graph_service: Arc<GS>,
    tool_router: ToolRouter<Self>,
}

#[tool_router]
impl<GS: GraphService> GraphServiceHandler<GS> {
    pub fn new(graph_service: GS) -> Self {
        Self {
            graph_service: Arc::new(graph_service),
            tool_router: Self::tool_router(),
        }
    }

    pub async fn create_entities(&self, request: CreateEntitiesRequest) -> Result<String, String> {
        match self.graph_service.create_entities(request.entities).await {
            Ok(created) => Ok(serde_json::to_string(&created).unwrap_or_else(|e| {
                format!("Created entities but failed to serialize response: {e}")
            })),
            Err(e) => Err(e.to_string()),
        }
    }

    pub async fn create_relations(
        &self,
        request: CreateRelationsRequest,
    ) -> Result<String, String> {
        match self.graph_service.create_relations(request.relations).await {
            Ok(created) => Ok(serde_json::to_string(&created).unwrap_or_else(|e| {
                format!("Created relations but failed to serialize response: {e}")
            })),
            Err(e) => Err(e.to_string()),
        }
    }

    #[tool(description = "Search for nodes in the knowledge graph by text query")]
    async fn search_nodes(
        &self,
        Parameters(request): Parameters<SearchNodesRequest>,
    ) -> Result<CallToolResult, McpError> {
        let result = self
            .graph_service
            .search_nodes(&request.query, request.limit)
            .await;

        match result {
            Ok(graph) => match serde_json::to_string(&*graph) {
                Ok(serialized) => Ok(CallToolResult::success(vec![Content::text(serialized)])),
                Err(_) => Err(McpError::internal_error("Failed to serialize graph", None)),
            },
            Err(_) => Err(McpError::internal_error("Search failed", None)),
        }
    }

    #[tool(description = "Get statistics about the knowledge graph")]
    async fn get_stats(&self) -> Result<CallToolResult, McpError> {
        match self.graph_service.get_stats().await {
            Ok((entities, relations)) => {
                let stats = GraphStats {
                    entities,
                    relations,
                };
                match serde_json::to_string(&stats) {
                    Ok(serialized) => Ok(CallToolResult::success(vec![Content::text(serialized)])),
                    Err(_) => Err(McpError::internal_error("Failed to serialize stats", None)),
                }
            }
            Err(_) => Err(McpError::internal_error("Failed to get stats", None)),
        }
    }

    #[tool(description = "Read the entire knowledge graph")]
    async fn read_graph(&self) -> Result<CallToolResult, McpError> {
        match self.graph_service.read_graph().await {
            Ok(graph) => match serde_json::to_string(&*graph) {
                Ok(serialized) => Ok(CallToolResult::success(vec![Content::text(serialized)])),
                Err(_) => Err(McpError::internal_error("Failed to serialize graph", None)),
            },
            Err(_) => Err(McpError::internal_error("Failed to read graph", None)),
        }
    }

    #[tool(description = "Add new observations to existing entities in the knowledge graph")]
    async fn add_observations(
        &self,
        Parameters(request): Parameters<AddObservationsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let observations: Vec<(String, Vec<String>)> = request
            .observations
            .into_iter()
            .map(|o| (o.entity_name, o.contents))
            .collect();

        match self.graph_service.add_observations(observations).await {
            Ok(results) => {
                let formatted_results: Vec<ObservationRequest> = results
                    .into_iter()
                    .map(|(entity_name, contents)| ObservationRequest {
                        entity_name,
                        contents,
                    })
                    .collect();

                match serde_json::to_string(&formatted_results) {
                    Ok(serialized) => Ok(CallToolResult::success(vec![Content::text(serialized)])),
                    Err(_) => Err(McpError::internal_error(
                        "Failed to serialize observations",
                        None,
                    )),
                }
            }
            Err(_) => Err(McpError::internal_error("Failed to add observations", None)),
        }
    }

    #[tool(
        description = "Delete multiple entities and their associated relations from the knowledge graph"
    )]
    async fn delete_entities(
        &self,
        Parameters(request): Parameters<DeleteEntitiesRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self
            .graph_service
            .delete_entities(request.entity_names)
            .await
        {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text(
                "Entities deleted successfully".to_string(),
            )])),
            Err(_) => Err(McpError::internal_error("Failed to delete entities", None)),
        }
    }

    #[tool(description = "Delete specific observations from entities in the knowledge graph")]
    async fn delete_observations(
        &self,
        Parameters(request): Parameters<DeleteObservationsRequest>,
    ) -> Result<CallToolResult, McpError> {
        let deletions: Vec<(String, Vec<String>)> = request
            .deletions
            .into_iter()
            .map(|d| (d.entity_name, d.observations))
            .collect();

        match self.graph_service.delete_observations(deletions).await {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text(
                "Observations deleted successfully".to_string(),
            )])),
            Err(_) => Err(McpError::internal_error(
                "Failed to delete observations",
                None,
            )),
        }
    }

    #[tool(description = "Delete multiple relations from the knowledge graph")]
    async fn delete_relations(
        &self,
        Parameters(request): Parameters<DeleteRelationsRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self.graph_service.delete_relations(request.relations).await {
            Ok(_) => Ok(CallToolResult::success(vec![Content::text(
                "Relations deleted successfully".to_string(),
            )])),
            Err(_) => Err(McpError::internal_error("Failed to delete relations", None)),
        }
    }

    #[tool(description = "Open specific nodes in the knowledge graph by their names")]
    async fn open_nodes(
        &self,
        Parameters(request): Parameters<OpenNodesRequest>,
    ) -> Result<CallToolResult, McpError> {
        match self.graph_service.open_nodes(request.names).await {
            Ok(graph) => match serde_json::to_string(&*graph) {
                Ok(serialized) => Ok(CallToolResult::success(vec![Content::text(serialized)])),
                Err(_) => Err(McpError::internal_error("Failed to serialize graph", None)),
            },
            Err(_) => Err(McpError::internal_error("Failed to open nodes", None)),
        }
    }
}

#[tool_handler]
impl<GS: GraphService> ServerHandler for GraphServiceHandler<GS> {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some("knowledge graph service".into()),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}
