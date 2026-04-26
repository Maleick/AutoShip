use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use arrow_array::{
    Array, FixedSizeListArray, Float32Array, Int32Array, Int64Array,
    RecordBatch, RecordBatchIterator, StringArray,
    types::Float32Type,
};
use arrow_schema::{DataType, Field, Schema};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use futures::TryStreamExt;
use lancedb::{Table, connect, index::Index, query::{ExecutableQuery, QueryBase}};
use serde::{Deserialize, Serialize};
use tokio::runtime::Builder;

use textquest_common::types::ClientId;

pub const SEMANTIC_EMBEDDING_DIM: usize = 384;
const SEMANTIC_TABLE_NAME: &str = "semantic";
const MERGE_SIMILARITY_THRESHOLD: f32 = 0.88;
const QUERY_PREFIX: &str = "query: ";
const PASSAGE_PREFIX: &str = "passage: ";

/// Search constraints for semantic recall.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SemanticMemoryFilter {
    /// Restrict results to a zone if one is known.
    pub zone: Option<String>,
    /// Restrict results to memories with at least this much party overlap.
    pub min_party_overlap: Option<usize>,
}

/// A semantic memory row stored in LanceDB.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SemanticMemoryRow {
    /// Stable row identifier.
    pub id: String,
    /// Character that owns the memory.
    pub character_id: ClientId,
    /// Natural-language memory text.
    pub content: String,
    /// Serialized tags for the memory.
    pub tags: Vec<String>,
    /// Optional session/debrief source.
    pub source_session_id: Option<String>,
    /// Creation timestamp in unix seconds.
    pub created_ts: i64,
    /// Optional zone context.
    pub zone: Option<String>,
    /// Approximate number of party members in overlap.
    pub party_overlap: usize,
    /// How many merges have been folded into this row.
    pub merge_count: u32,
    /// Stored embedding vector.
    pub embedding: Vec<f32>,
    /// Relevance score from recall queries.
    #[serde(skip)]
    pub score: Option<f32>,
}

impl SemanticMemoryRow {
    fn to_prompt_line(&self) -> String {
        let mut parts = vec![self.content.clone()];
        if !self.tags.is_empty() {
            parts.push(format!("tags: {}", self.tags.join(",")));
        }
        if let Some(zone) = &self.zone {
            parts.push(format!("zone: {zone}"));
        }
        if self.party_overlap > 0 {
            parts.push(format!("party_overlap: {}", self.party_overlap));
        }
        if self.merge_count > 0 {
            parts.push(format!("merged: {}", self.merge_count));
        }
        parts.join(" | ")
    }
}

pub trait Embedder: Send + Sync {
    fn embed_document(&self, text: &str) -> Vec<f32>;
    fn embed_query(&self, text: &str) -> Vec<f32>;
}

struct FastEmbedBackend {
    model: Mutex<FastEmbedState>,
}

enum FastEmbedState {
    Uninitialized,
    Ready(Box<TextEmbedding>),
    Fallback,
}

impl FastEmbedBackend {
    fn new() -> Self {
        Self {
            model: Mutex::new(FastEmbedState::Uninitialized),
        }
    }

    fn embed_with_prefix(&self, prefix: &str, text: &str) -> Vec<f32> {
        let mut guard = self.model.lock().expect("embedding mutex poisoned");
        match &mut *guard {
            FastEmbedState::Ready(model) => match model.embed([format!("{prefix}{text}")], None) {
                Ok(mut outputs) => outputs.pop().unwrap_or_else(|| hash_embedding(text)),
                Err(err) => {
                    tracing::warn!(error = %err, "semantic_memory: fastembed query failed; using fallback embedding");
                    *guard = FastEmbedState::Fallback;
                    hash_embedding(text)
                }
            },
            FastEmbedState::Fallback => hash_embedding(text),
            FastEmbedState::Uninitialized => match TextEmbedding::try_new(
                InitOptions::new(EmbeddingModel::BGESmallENV15).with_show_download_progress(false),
            ) {
                Ok(model) => {
                    *guard = FastEmbedState::Ready(Box::new(model));
                    match &mut *guard {
                        FastEmbedState::Ready(model) => {
                            match model.embed([format!("{prefix}{text}")], None) {
                                Ok(mut outputs) => {
                                    outputs.pop().unwrap_or_else(|| hash_embedding(text))
                                }
                                Err(err) => {
                                    tracing::warn!(
                                        error = %err,
                                        "semantic_memory: fastembed query failed after init; using fallback embedding"
                                    );
                                    *guard = FastEmbedState::Fallback;
                                    hash_embedding(text)
                                }
                            }
                        }
                        _ => hash_embedding(text),
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        error = %err,
                        "semantic_memory: fastembed init failed; using fallback embeddings"
                    );
                    *guard = FastEmbedState::Fallback;
                    hash_embedding(text)
                }
            },
        }
    }
}

impl Embedder for FastEmbedBackend {
    fn embed_document(&self, text: &str) -> Vec<f32> {
        self.embed_with_prefix(PASSAGE_PREFIX, text)
    }

    fn embed_query(&self, text: &str) -> Vec<f32> {
        self.embed_with_prefix(QUERY_PREFIX, text)
    }
}

#[allow(dead_code)]
struct HashEmbedBackend;

impl Embedder for HashEmbedBackend {
    fn embed_document(&self, text: &str) -> Vec<f32> {
        hash_embedding(&format!("{PASSAGE_PREFIX}{text}"))
    }

    fn embed_query(&self, text: &str) -> Vec<f32> {
        hash_embedding(&format!("{QUERY_PREFIX}{text}"))
    }
}

static DEFAULT_EMBEDDER: OnceLock<Arc<dyn Embedder>> = OnceLock::new();

fn default_embedder() -> Arc<dyn Embedder> {
    DEFAULT_EMBEDDER
        .get_or_init(|| Arc::new(FastEmbedBackend::new()) as Arc<dyn Embedder>)
        .clone()
}

fn hash_embedding(text: &str) -> Vec<f32> {
    let mut vector = vec![0.0f32; SEMANTIC_EMBEDDING_DIM];
    for token in text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
    {
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in token.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        let idx = (hash as usize) % SEMANTIC_EMBEDDING_DIM;
        vector[idx] += 1.0;
    }

    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut vector {
            *value /= norm;
        }
    }
    vector
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;
    for (&lhs, &rhs) in a.iter().zip(b.iter()) {
        dot += lhs * rhs;
        norm_a += lhs * lhs;
        norm_b += rhs * rhs;
    }
    if norm_a <= f32::EPSILON || norm_b <= f32::EPSILON {
        0.0
    } else {
        dot / (norm_a.sqrt() * norm_b.sqrt())
    }
}

fn normalize_content(content: &str) -> String {
    content
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn current_unix_ts() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs() as i64)
}

fn escape_sql_string(value: &str) -> String {
    value.replace('\'', "''")
}

fn semantic_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("character_id", DataType::Int64, false),
        Field::new("content", DataType::Utf8, false),
        Field::new(
            "embedding",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                SEMANTIC_EMBEDDING_DIM as i32,
            ),
            false,
        ),
        Field::new("tags", DataType::Utf8, false),
        Field::new("source_session_id", DataType::Utf8, true),
        Field::new("created_ts", DataType::Int64, false),
        Field::new("zone", DataType::Utf8, true),
        Field::new("party_overlap", DataType::Int32, false),
        Field::new("merge_count", DataType::Int32, false),
    ]))
}

fn semantic_db_path(sqlite_path: &Path) -> PathBuf {
    let mut path = sqlite_path.to_path_buf();
    path.set_extension("semantic.lancedb");
    path
}

fn row_to_batch(row: &SemanticMemoryRow) -> Result<RecordBatch> {
    let schema = semantic_schema();
    let embedding: Vec<Option<f32>> = row.embedding.iter().copied().map(Some).collect();
    let embedding_array = FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
        std::iter::once(Some(embedding)),
        SEMANTIC_EMBEDDING_DIM as i32,
    );
    RecordBatch::try_new(
        schema,
        vec![
            Arc::new(StringArray::from(vec![row.id.as_str()])),
            Arc::new(Int64Array::from(vec![row.character_id as i64])),
            Arc::new(StringArray::from(vec![row.content.as_str()])),
            Arc::new(embedding_array),
            Arc::new(StringArray::from(vec![serde_json::to_string(&row.tags)?])),
            Arc::new(StringArray::from(vec![row.source_session_id.as_deref()])),
            Arc::new(Int64Array::from(vec![row.created_ts])),
            Arc::new(StringArray::from(vec![row.zone.as_deref()])),
            Arc::new(Int32Array::from(vec![row.party_overlap as i32])),
            Arc::new(Int32Array::from(vec![row.merge_count as i32])),
        ],
    )
    .context("failed to build semantic memory record batch")
}

fn batch_rows(batch: &RecordBatch) -> Result<Vec<SemanticMemoryRow>> {
    let schema = batch.schema();
    let id_idx = schema.index_of("id")?;
    let character_idx = schema.index_of("character_id")?;
    let content_idx = schema.index_of("content")?;
    let embedding_idx = schema.index_of("embedding")?;
    let tags_idx = schema.index_of("tags")?;
    let session_idx = schema.index_of("source_session_id")?;
    let ts_idx = schema.index_of("created_ts")?;
    let zone_idx = schema.index_of("zone")?;
    let overlap_idx = schema.index_of("party_overlap")?;
    let merge_idx = schema.index_of("merge_count")?;

    let ids = batch
        .column(id_idx)
        .as_any()
        .downcast_ref::<StringArray>()
        .context("semantic id column has unexpected type")?;
    let character_ids = batch
        .column(character_idx)
        .as_any()
        .downcast_ref::<Int64Array>()
        .context("semantic character_id column has unexpected type")?;
    let contents = batch
        .column(content_idx)
        .as_any()
        .downcast_ref::<StringArray>()
        .context("semantic content column has unexpected type")?;
    let embeddings = batch
        .column(embedding_idx)
        .as_any()
        .downcast_ref::<FixedSizeListArray>()
        .context("semantic embedding column has unexpected type")?;
    let embedding_values = embeddings
        .values()
        .as_any()
        .downcast_ref::<Float32Array>()
        .context("semantic embedding values column has unexpected type")?;
    let tags = batch
        .column(tags_idx)
        .as_any()
        .downcast_ref::<StringArray>()
        .context("semantic tags column has unexpected type")?;
    let sessions = batch
        .column(session_idx)
        .as_any()
        .downcast_ref::<StringArray>()
        .context("semantic source_session_id column has unexpected type")?;
    let timestamps = batch
        .column(ts_idx)
        .as_any()
        .downcast_ref::<Int64Array>()
        .context("semantic created_ts column has unexpected type")?;
    let zones = batch
        .column(zone_idx)
        .as_any()
        .downcast_ref::<StringArray>()
        .context("semantic zone column has unexpected type")?;
    let overlaps = batch
        .column(overlap_idx)
        .as_any()
        .downcast_ref::<Int32Array>()
        .context("semantic party_overlap column has unexpected type")?;
    let merges = batch
        .column(merge_idx)
        .as_any()
        .downcast_ref::<Int32Array>()
        .context("semantic merge_count column has unexpected type")?;

    let embedding_width = embeddings.value_length() as usize;
    let mut rows = Vec::with_capacity(batch.num_rows());
    for row_index in 0..batch.num_rows() {
        let embedding_start = row_index * embedding_width;
        let embedding_end = embedding_start + embedding_width;
        let embedding = (embedding_start..embedding_end)
            .map(|idx| embedding_values.value(idx))
            .collect::<Vec<_>>();
        let tags = serde_json::from_str(tags.value(row_index)).unwrap_or_default();
        rows.push(SemanticMemoryRow {
            id: ids.value(row_index).to_string(),
            character_id: character_ids.value(row_index) as ClientId,
            content: contents.value(row_index).to_string(),
            tags,
            source_session_id: if sessions.is_null(row_index) {
                None
            } else {
                Some(sessions.value(row_index).to_string())
            },
            created_ts: timestamps.value(row_index),
            zone: if zones.is_null(row_index) {
                None
            } else {
                Some(zones.value(row_index).to_string())
            },
            party_overlap: overlaps.value(row_index).max(0) as usize,
            merge_count: merges.value(row_index).max(0) as u32,
            embedding,
            score: None,
        });
    }

    Ok(rows)
}

fn filter_clause(character_id: ClientId, filter: &SemanticMemoryFilter) -> String {
    let mut clauses = vec![format!("character_id = {}", character_id as i64)];
    if let Some(zone) = &filter.zone {
        clauses.push(format!("zone = '{}'", escape_sql_string(zone)));
    }
    if let Some(min_party_overlap) = filter.min_party_overlap {
        clauses.push(format!("party_overlap >= {}", min_party_overlap as i64));
    }
    clauses.join(" AND ")
}

pub fn build_memory_blob(
    event_type: &str,
    content: &str,
    zone: Option<&str>,
    mood: &str,
    kind: &str,
) -> String {
    let mut parts = vec![
        format!("kind={kind}"),
        format!("type={event_type}"),
        format!("content={content}"),
        format!("mood={mood}"),
    ];
    if let Some(zone) = zone {
        parts.push(format!("zone={zone}"));
    }
    parts.join("\n")
}

fn next_row_id(character_id: ClientId) -> String {
    format!("{character_id}-{}", current_unix_ts())
}

/// Embedded LanceDB-backed semantic memory store.
pub struct SemanticMemoryStore {
    runtime: tokio::runtime::Runtime,
    table: Arc<Table>,
    embedder: Arc<dyn Embedder>,
}

impl SemanticMemoryStore {
    /// Open a semantic memory database adjacent to the given SQLite path.
    pub fn open(sqlite_path: &Path) -> Result<Self> {
        let db_path = semantic_db_path(sqlite_path);
        Self::open_at(db_path)
    }

    /// Open a semantic memory database in-memory for tests.
    pub fn open_in_memory() -> Result<Self> {
        Self::open_at(PathBuf::from("memory://"))
    }

    /// Open a semantic memory database with a custom embedder.
    pub fn open_with_embedder(sqlite_path: &Path, embedder: Arc<dyn Embedder>) -> Result<Self> {
        let db_path = semantic_db_path(sqlite_path);
        Self::open_at_with_embedder(db_path, embedder)
    }

    fn open_at(db_path: PathBuf) -> Result<Self> {
        Self::open_at_with_embedder(db_path, default_embedder())
    }

    fn open_at_with_embedder(db_path: PathBuf, embedder: Arc<dyn Embedder>) -> Result<Self> {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("failed to create runtime for semantic memory")?;

        let table = runtime.block_on(async {
            let db = connect(db_path.to_string_lossy().as_ref())
                .execute()
                .await
                .context("failed to connect semantic LanceDB")?;
            let schema = semantic_schema();
            let table = match db.open_table(SEMANTIC_TABLE_NAME).execute().await {
                Ok(table) => table,
                Err(_) => db
                    .create_empty_table(SEMANTIC_TABLE_NAME, schema.clone())
                    .execute()
                    .await
                    .context("failed to create semantic LanceDB table")?,
            };

            if let Err(err) = table.create_index(&["character_id"], Index::Auto).execute().await {
                tracing::debug!(error = %err, "semantic_memory: character_id index not created");
            }
            if let Err(err) = table.create_index(&["embedding"], Index::Auto).execute().await {
                tracing::debug!(error = %err, "semantic_memory: embedding index not created");
            }

            Ok::<_, anyhow::Error>(Arc::new(table))
        })?;

        Ok(Self {
            runtime,
            table,
            embedder,
        })
    }

    /// Store a session debrief or distilled fact.
    pub fn record_fact(
        &self,
        character_id: ClientId,
        content: &str,
        tags: &[&str],
        source_session_id: Option<&str>,
        zone: Option<&str>,
        party_overlap: usize,
    ) -> Result<SemanticMemoryRow> {
        self.record_internal(
            character_id,
            content,
            tags,
            source_session_id,
            zone,
            party_overlap,
            "fact",
        )
    }

    /// Store a session debrief.
    pub fn record_debrief(
        &self,
        character_id: ClientId,
        content: &str,
        tags: &[&str],
        source_session_id: Option<&str>,
        zone: Option<&str>,
        party_overlap: usize,
    ) -> Result<SemanticMemoryRow> {
        self.record_internal(
            character_id,
            content,
            tags,
            source_session_id,
            zone,
            party_overlap,
            "debrief",
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn record_internal(
        &self,
        character_id: ClientId,
        content: &str,
        tags: &[&str],
        source_session_id: Option<&str>,
        zone: Option<&str>,
        party_overlap: usize,
        kind: &str,
    ) -> Result<SemanticMemoryRow> {
        let content = normalize_content(content);
        if content.is_empty() {
            return Err(anyhow::anyhow!("semantic memory content cannot be empty"));
        }
        let mut tags_vec = tags.iter().map(|tag| (*tag).to_string()).collect::<Vec<_>>();
        if !tags_vec.iter().any(|tag| tag == kind) {
            tags_vec.push(kind.to_string());
        }
        let embedding = self.embedder.embed_document(&content);
        let current = SemanticMemoryRow {
            id: next_row_id(character_id),
            character_id,
            content: content.clone(),
            tags: tags_vec.clone(),
            source_session_id: source_session_id.map(ToOwned::to_owned),
            created_ts: current_unix_ts(),
            zone: zone.map(ToOwned::to_owned),
            party_overlap,
            merge_count: 0,
            embedding,
            score: None,
        };

        let maybe_existing = self
            .recall_internal(
                character_id,
                &content,
                SemanticMemoryFilter {
                    zone: zone.map(ToOwned::to_owned),
                    min_party_overlap: Some(party_overlap),
                },
                1,
            )?
            .into_iter()
            .next()
            .filter(|row| row.score.unwrap_or(0.0) >= MERGE_SIMILARITY_THRESHOLD);

        let row = if let Some(existing) = maybe_existing {
            let merged = merge_semantic_rows(existing, current);
            self.runtime.block_on(self.replace_row(&merged))?;
            merged
        } else {
            self.runtime.block_on(self.insert_row(&current))?;
            current
        };

        Ok(row)
    }

    /// Recall semantic memories using a semantic query and metadata filter.
    pub fn recall(
        &self,
        character_id: ClientId,
        query: &str,
        filter: SemanticMemoryFilter,
        top_k: usize,
    ) -> Result<Vec<SemanticMemoryRow>> {
        self.recall_internal(character_id, query, filter, top_k)
    }

    /// Recall the most recent semantic memories without a semantic query.
    pub fn recall_recent(&self, character_id: ClientId, top_k: usize) -> Result<Vec<SemanticMemoryRow>> {
        self.recall_recent_internal(character_id, top_k)
    }

    fn recall_internal(
        &self,
        character_id: ClientId,
        query: &str,
        filter: SemanticMemoryFilter,
        top_k: usize,
    ) -> Result<Vec<SemanticMemoryRow>> {
        let query_embedding = self.embedder.embed_query(query);
        let clause = filter_clause(character_id, &filter);
        let table = self.table.clone();
        let rows = self.runtime.block_on(async move {
            let stream = table
                .query()
                .only_if(&clause)
                .execute()
                .await
                .context("failed to query semantic LanceDB")?;
            let batches = stream
                .try_collect::<Vec<RecordBatch>>()
                .await
                .context("failed to read semantic LanceDB rows")?;
            let mut rows = Vec::new();
            for batch in &batches {
                rows.extend(batch_rows(batch)?);
            }
            Ok::<_, anyhow::Error>(rows)
        })?;

        let mut rows = rows
            .into_iter()
            .map(|mut row| {
                row.score = Some(cosine_similarity(&query_embedding, &row.embedding));
                row
            })
            .collect::<Vec<_>>();

        rows.sort_by(|lhs, rhs| {
            rhs.score
                .partial_cmp(&lhs.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| rhs.created_ts.cmp(&lhs.created_ts))
        });
        rows.truncate(top_k);
        Ok(rows)
    }

    fn recall_recent_internal(
        &self,
        character_id: ClientId,
        top_k: usize,
    ) -> Result<Vec<SemanticMemoryRow>> {
        let clause = format!("character_id = {}", character_id as i64);
        let table = self.table.clone();
        let rows = self.runtime.block_on(async move {
            let stream = table
                .query()
                .only_if(&clause)
                .execute()
                .await
                .context("failed to query semantic LanceDB")?;
            let batches = stream
                .try_collect::<Vec<RecordBatch>>()
                .await
                .context("failed to read semantic LanceDB rows")?;
            let mut rows = Vec::new();
            for batch in &batches {
                rows.extend(batch_rows(batch)?);
            }
            Ok::<_, anyhow::Error>(rows)
        })?;

        let mut rows = rows;
        rows.sort_by(|lhs, rhs| {
            rhs.created_ts
                .cmp(&lhs.created_ts)
                .then_with(|| rhs.merge_count.cmp(&lhs.merge_count))
        });
        rows.truncate(top_k);
        Ok(rows)
    }

    async fn insert_row(&self, row: &SemanticMemoryRow) -> Result<()> {
        let batch = row_to_batch(row)?;
        let batches = RecordBatchIterator::new(vec![batch].into_iter().map(Ok), semantic_schema());
        self.table
            .add(Box::new(batches) as Box<dyn arrow_array::RecordBatchReader + Send>)
            .execute()
            .await
            .context("failed to insert semantic memory row")?;
        Ok(())
    }

    async fn replace_row(&self, row: &SemanticMemoryRow) -> Result<()> {
        let filter = format!("id = '{}'", escape_sql_string(&row.id));
        self.table
            .delete(filter.as_str())
            .await
            .context("failed to delete merged semantic memory row")?;
        self.insert_row(row).await
    }
}

fn merge_semantic_rows(
    existing: SemanticMemoryRow,
    mut incoming: SemanticMemoryRow,
) -> SemanticMemoryRow {
    let mut tags = existing.tags.into_iter().collect::<HashSet<_>>();
    tags.extend(incoming.tags.drain(..));

    let merged_content = if existing.content == incoming.content {
        existing.content.clone()
    } else if existing.content.len() <= incoming.content.len() {
        format!("{}; {}", existing.content, incoming.content)
    } else {
        format!("{}; {}", incoming.content, existing.content)
    };

    let merged_embedding = default_embedder().embed_document(&merged_content);
    SemanticMemoryRow {
        id: existing.id,
        character_id: existing.character_id,
        content: merged_content,
        tags: tags.into_iter().collect(),
        source_session_id: incoming.source_session_id.or(existing.source_session_id),
        created_ts: existing.created_ts.max(incoming.created_ts),
        zone: incoming.zone.or(existing.zone),
        party_overlap: existing.party_overlap.max(incoming.party_overlap),
        merge_count: existing.merge_count.saturating_add(incoming.merge_count).saturating_add(1),
        embedding: merged_embedding,
        score: None,
    }
}

/// Embed a semantic memory query with the default backend.
pub fn embed_query(text: &str) -> Vec<f32> {
    default_embedder().embed_query(text)
}

/// Embed a semantic memory document with the default backend.
pub fn embed_document(text: &str) -> Vec<f32> {
    default_embedder().embed_document(text)
}

/// Build a readable prompt snippet for a semantic memory row.
pub fn prompt_snippet(row: &SemanticMemoryRow) -> String {
    row.to_prompt_line()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestEmbedder;

    impl Embedder for TestEmbedder {
        fn embed_document(&self, text: &str) -> Vec<f32> {
            match text {
                t if t.contains("pull-from-camp") => {
                    let mut v = vec![0.0; SEMANTIC_EMBEDDING_DIM];
                    v[0] = 1.0;
                    v
                }
                t if t.contains("bow-pulls") => {
                    let mut v = vec![0.0; SEMANTIC_EMBEDDING_DIM];
                    v[0] = 0.95;
                    v
                }
                _ => {
                    let mut v = vec![0.0; SEMANTIC_EMBEDDING_DIM];
                    v[1] = 1.0;
                    v
                }
            }
        }

        fn embed_query(&self, text: &str) -> Vec<f32> {
            self.embed_document(text)
        }
    }

    fn test_store() -> SemanticMemoryStore {
        SemanticMemoryStore::open_with_embedder(
            Path::new("memory://"),
            Arc::new(TestEmbedder) as Arc<dyn Embedder>,
        )
        .unwrap()
    }

    #[test]
    fn hash_embedding_has_expected_dimension() {
        let vec = hash_embedding("Tank prefers pull-from-camp");
        assert_eq!(vec.len(), SEMANTIC_EMBEDDING_DIM);
        assert!(vec.iter().any(|value| *value > 0.0));
    }

    #[test]
    fn record_fact_merges_overlapping_content() {
        let store = test_store();
        let first = store
            .record_fact(
                1,
                "Tank prefers pull-from-camp",
                &["tank", "camp"],
                Some("session-1"),
                Some("lguk"),
                4,
            )
            .unwrap();
        assert_eq!(first.merge_count, 0);

        let merged = store
            .record_fact(
                1,
                "Tank likes bow-pulls",
                &["tank", "pulls"],
                Some("session-2"),
                Some("lguk"),
                4,
            )
            .unwrap();

        assert!(merged.content.contains("pull-from-camp"));
        assert!(merged.content.contains("bow-pulls"));
        assert!(merged.merge_count >= 1);
        assert!(merged.tags.iter().any(|tag| tag == "camp"));
        assert!(merged.tags.iter().any(|tag| tag == "pulls"));
    }

    #[test]
    fn recall_recent_orders_by_timestamp() {
        let store = test_store();
        store
            .record_fact(7, "old fact", &["old"], None, Some("permafrost"), 2)
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(5));
        store
            .record_fact(7, "new fact", &["new"], None, Some("permafrost"), 2)
            .unwrap();

        let rows = store.recall_recent(7, 1).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].content, "new fact");
    }
}
