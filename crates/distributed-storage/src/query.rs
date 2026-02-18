//! Query capabilities for distributed storage
//!
//! This module provides SQL-like query capabilities using sled's range iterators.
//! It supports block-based filtering, pagination, and fluent query construction.

use bincode::Options;
use serde::{Deserialize, Serialize};

use crate::store::{StorageKey, StoredValue};

/// Maximum size for deserializing query cursor data (1MB).
/// This limit prevents DoS attacks from malformed data causing excessive memory allocation.
/// Cursors are small structures, so 1MB is more than sufficient.
const MAX_CURSOR_SIZE: u64 = 1024 * 1024;

/// Create bincode options with size limit for safe deserialization.
/// Uses fixint encoding and allows trailing bytes for compatibility with `bincode::serialize()`.
fn bincode_options() -> impl Options {
    bincode::options()
        .with_limit(MAX_CURSOR_SIZE)
        .with_fixint_encoding()
        .allow_trailing_bytes()
}

/// Result of a query operation with pagination support
#[derive(Clone, Debug)]
pub struct QueryResult {
    /// Matching items as key-value pairs
    pub items: Vec<(StorageKey, StoredValue)>,
    /// Total count of items matching the filter (before limit)
    pub total_count: Option<u64>,
    /// Current page offset
    pub offset: usize,
    /// Requested limit
    pub limit: usize,
    /// Whether there are more results beyond the current page
    pub has_more: bool,
    /// Continuation token for fetching the next page
    pub next_cursor: Option<QueryCursor>,
}

impl QueryResult {
    /// Create a new empty query result
    pub fn empty(limit: usize) -> Self {
        Self {
            items: Vec::new(),
            total_count: None,
            offset: 0,
            limit,
            has_more: false,
            next_cursor: None,
        }
    }

    /// Create a query result from items
    pub fn new(
        items: Vec<(StorageKey, StoredValue)>,
        offset: usize,
        limit: usize,
        has_more: bool,
    ) -> Self {
        Self {
            items,
            total_count: None,
            offset,
            limit,
            has_more,
            next_cursor: None,
        }
    }

    /// Set the total count
    pub fn with_total_count(mut self, count: u64) -> Self {
        self.total_count = Some(count);
        self
    }

    /// Set the continuation cursor
    pub fn with_cursor(mut self, cursor: QueryCursor) -> Self {
        self.next_cursor = Some(cursor);
        self
    }

    /// Get the number of items in this result
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if the result is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

/// Cursor for pagination in queries
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryCursor {
    /// Last block_id seen
    pub last_block_id: Option<u64>,
    /// Last key hash seen (for tie-breaking within same block)
    pub last_key_hash: Option<Vec<u8>>,
    /// Namespace being queried
    pub namespace: String,
}

impl QueryCursor {
    /// Create a new cursor
    pub fn new(namespace: &str) -> Self {
        Self {
            last_block_id: None,
            last_key_hash: None,
            namespace: namespace.to_string(),
        }
    }

    /// Create a cursor from the last item in a result
    pub fn from_last_item(namespace: &str, block_id: u64, key: &StorageKey) -> Self {
        Self {
            last_block_id: Some(block_id),
            last_key_hash: Some(key.hash().to_vec()),
            namespace: namespace.to_string(),
        }
    }

    /// Encode cursor to bytes for transport
    pub fn to_bytes(&self) -> Vec<u8> {
        bincode::serialize(self).unwrap_or_default()
    }

    /// Decode cursor from bytes with size limit protection.
    /// Limits deserialization to MAX_CURSOR_SIZE bytes to prevent DoS via memory exhaustion.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        bincode_options().deserialize(bytes).ok()
    }

    /// Encode cursor to base64 string
    pub fn to_base64(&self) -> String {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(self.to_bytes())
    }

    /// Decode cursor from base64 string
    pub fn from_base64(s: &str) -> Option<Self> {
        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD.decode(s).ok()?;
        Self::from_bytes(&bytes)
    }
}

/// Filter types for queries
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum QueryFilter {
    /// Match entries where block_id is less than the specified value
    BlockBefore(u64),
    /// Match entries where block_id is greater than the specified value
    BlockAfter(u64),
    /// Match entries where block_id is within a range (inclusive)
    BlockRange { start: u64, end: u64 },
    /// Match entries where created_at timestamp is before the specified value
    CreatedBefore(i64),
    /// Match entries where created_at timestamp is after the specified value
    CreatedAfter(i64),
    /// Match entries with a specific key prefix within the namespace
    KeyPrefix(Vec<u8>),
    /// Combine multiple filters with AND logic
    And(Vec<QueryFilter>),
    /// Combine multiple filters with OR logic
    Or(Vec<QueryFilter>),
}

impl QueryFilter {
    /// Check if an entry matches this filter
    pub fn matches(&self, block_id: Option<u64>, created_at: i64, key: &[u8]) -> bool {
        match self {
            QueryFilter::BlockBefore(max_block) => block_id.is_none_or(|b| b < *max_block),
            QueryFilter::BlockAfter(min_block) => block_id.is_some_and(|b| b > *min_block),
            QueryFilter::BlockRange { start, end } => {
                block_id.is_some_and(|b| b >= *start && b <= *end)
            }
            QueryFilter::CreatedBefore(timestamp) => created_at < *timestamp,
            QueryFilter::CreatedAfter(timestamp) => created_at > *timestamp,
            QueryFilter::KeyPrefix(prefix) => key.starts_with(prefix),
            QueryFilter::And(filters) => {
                filters.iter().all(|f| f.matches(block_id, created_at, key))
            }
            QueryFilter::Or(filters) => {
                filters.iter().any(|f| f.matches(block_id, created_at, key))
            }
        }
    }
}

/// Builder for constructing queries with a fluent API
#[derive(Clone, Debug)]
pub struct QueryBuilder {
    namespace: String,
    filters: Vec<QueryFilter>,
    limit: usize,
    offset: usize,
    cursor: Option<QueryCursor>,
    include_count: bool,
    order_ascending: bool,
}

impl QueryBuilder {
    /// Create a new query builder for a namespace
    pub fn new(namespace: &str) -> Self {
        Self {
            namespace: namespace.to_string(),
            filters: Vec::new(),
            limit: 100,
            offset: 0,
            cursor: None,
            include_count: false,
            order_ascending: true,
        }
    }

    /// Add a filter for entries before a specific block
    pub fn before_block(mut self, block_id: u64) -> Self {
        self.filters.push(QueryFilter::BlockBefore(block_id));
        self
    }

    /// Add a filter for entries after a specific block
    pub fn after_block(mut self, block_id: u64) -> Self {
        self.filters.push(QueryFilter::BlockAfter(block_id));
        self
    }

    /// Add a filter for entries within a block range (inclusive)
    pub fn block_range(mut self, start: u64, end: u64) -> Self {
        self.filters.push(QueryFilter::BlockRange { start, end });
        self
    }

    /// Add a filter for entries created before a timestamp
    pub fn created_before(mut self, timestamp: i64) -> Self {
        self.filters.push(QueryFilter::CreatedBefore(timestamp));
        self
    }

    /// Add a filter for entries created after a timestamp
    pub fn created_after(mut self, timestamp: i64) -> Self {
        self.filters.push(QueryFilter::CreatedAfter(timestamp));
        self
    }

    /// Add a filter for entries with a key prefix
    pub fn key_prefix(mut self, prefix: impl Into<Vec<u8>>) -> Self {
        self.filters.push(QueryFilter::KeyPrefix(prefix.into()));
        self
    }

    /// Set the maximum number of results to return
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    /// Set the offset for pagination (alternative to cursor)
    pub fn offset(mut self, offset: usize) -> Self {
        self.offset = offset;
        self
    }

    /// Set a cursor for pagination
    pub fn cursor(mut self, cursor: QueryCursor) -> Self {
        self.cursor = Some(cursor);
        self
    }

    /// Include the total count in results (may be slower)
    pub fn with_count(mut self) -> Self {
        self.include_count = true;
        self
    }

    /// Order results in ascending order (default)
    pub fn ascending(mut self) -> Self {
        self.order_ascending = true;
        self
    }

    /// Order results in descending order
    pub fn descending(mut self) -> Self {
        self.order_ascending = false;
        self
    }

    /// Get the namespace
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Get all filters
    pub fn filters(&self) -> &[QueryFilter] {
        &self.filters
    }

    /// Get the limit
    pub fn get_limit(&self) -> usize {
        self.limit
    }

    /// Get the offset
    pub fn get_offset(&self) -> usize {
        self.offset
    }

    /// Get the cursor
    pub fn get_cursor(&self) -> Option<&QueryCursor> {
        self.cursor.as_ref()
    }

    /// Check if count should be included
    pub fn should_include_count(&self) -> bool {
        self.include_count
    }

    /// Check if order is ascending
    pub fn is_ascending(&self) -> bool {
        self.order_ascending
    }

    /// Build a combined filter from all added filters
    pub fn build_filter(&self) -> Option<QueryFilter> {
        if self.filters.is_empty() {
            None
        } else if self.filters.len() == 1 {
            Some(self.filters[0].clone())
        } else {
            Some(QueryFilter::And(self.filters.clone()))
        }
    }
}

/// Generate a block index key for efficient range queries
///
/// Format: `namespace:block_id(8 bytes BE):key_hash(32 bytes)`
/// Using big-endian ensures proper lexicographic ordering of block IDs.
pub fn block_index_key(namespace: &str, block_id: u64, key: &StorageKey) -> Vec<u8> {
    let mut result = Vec::with_capacity(namespace.len() + 1 + 8 + 1 + 32);
    result.extend_from_slice(namespace.as_bytes());
    result.push(b':');
    result.extend_from_slice(&block_id.to_be_bytes());
    result.push(b':');
    result.extend_from_slice(&key.hash());
    result
}

/// Parse a block index key back to its components
///
/// Returns (namespace, block_id, key_hash) if successful
pub fn parse_block_index_key(key: &[u8]) -> Option<(String, u64, [u8; 32])> {
    // Find the first colon to separate namespace
    let first_colon = key.iter().position(|&b| b == b':')?;
    let namespace = std::str::from_utf8(&key[..first_colon]).ok()?.to_string();

    // After namespace:, we have 8 bytes for block_id, then :, then 32 bytes for hash
    let rest = &key[first_colon + 1..];
    if rest.len() < 8 + 1 + 32 {
        return None;
    }

    let block_id = u64::from_be_bytes(rest[..8].try_into().ok()?);

    // Verify the separator
    if rest[8] != b':' {
        return None;
    }

    let mut key_hash = [0u8; 32];
    key_hash.copy_from_slice(&rest[9..41]);

    Some((namespace, block_id, key_hash))
}

/// Generate the start key for a block range query
pub fn block_range_start(namespace: &str, start_block: u64) -> Vec<u8> {
    let mut result = Vec::with_capacity(namespace.len() + 1 + 8);
    result.extend_from_slice(namespace.as_bytes());
    result.push(b':');
    result.extend_from_slice(&start_block.to_be_bytes());
    result
}

/// Generate the end key for a block range query (exclusive)
pub fn block_range_end(namespace: &str, end_block: u64) -> Vec<u8> {
    let mut result = Vec::with_capacity(namespace.len() + 1 + 8);
    result.extend_from_slice(namespace.as_bytes());
    result.push(b':');
    // Add 1 to make it exclusive, handle overflow
    let end = end_block.saturating_add(1);
    result.extend_from_slice(&end.to_be_bytes());
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_result_empty() {
        let result = QueryResult::empty(10);
        assert!(result.is_empty());
        assert_eq!(result.len(), 0);
        assert_eq!(result.limit, 10);
        assert!(!result.has_more);
    }

    #[test]
    fn test_query_cursor_serialization() {
        let cursor = QueryCursor::new("submissions");
        let bytes = cursor.to_bytes();
        let decoded = QueryCursor::from_bytes(&bytes).expect("should decode");
        assert_eq!(decoded.namespace, "submissions");
    }

    #[test]
    fn test_query_cursor_base64() {
        let cursor = QueryCursor::from_last_item(
            "submissions",
            1000,
            &StorageKey::new("submissions", "test-key"),
        );
        let encoded = cursor.to_base64();
        let decoded = QueryCursor::from_base64(&encoded).expect("should decode");
        assert_eq!(decoded.namespace, "submissions");
        assert_eq!(decoded.last_block_id, Some(1000));
        assert!(decoded.last_key_hash.is_some());
    }

    #[test]
    fn test_query_filter_block_before() {
        let filter = QueryFilter::BlockBefore(100);
        assert!(filter.matches(Some(50), 0, b"key"));
        assert!(filter.matches(Some(99), 0, b"key"));
        assert!(!filter.matches(Some(100), 0, b"key"));
        assert!(!filter.matches(Some(101), 0, b"key"));
        // None block_id matches (no block info available)
        assert!(filter.matches(None, 0, b"key"));
    }

    #[test]
    fn test_query_filter_block_after() {
        let filter = QueryFilter::BlockAfter(100);
        assert!(!filter.matches(Some(50), 0, b"key"));
        assert!(!filter.matches(Some(100), 0, b"key"));
        assert!(filter.matches(Some(101), 0, b"key"));
        // None block_id doesn't match
        assert!(!filter.matches(None, 0, b"key"));
    }

    #[test]
    fn test_query_filter_block_range() {
        let filter = QueryFilter::BlockRange {
            start: 50,
            end: 150,
        };
        assert!(!filter.matches(Some(49), 0, b"key"));
        assert!(filter.matches(Some(50), 0, b"key"));
        assert!(filter.matches(Some(100), 0, b"key"));
        assert!(filter.matches(Some(150), 0, b"key"));
        assert!(!filter.matches(Some(151), 0, b"key"));
    }

    #[test]
    fn test_query_filter_created_before() {
        let filter = QueryFilter::CreatedBefore(1000);
        assert!(filter.matches(None, 500, b"key"));
        assert!(filter.matches(None, 999, b"key"));
        assert!(!filter.matches(None, 1000, b"key"));
        assert!(!filter.matches(None, 1001, b"key"));
    }

    #[test]
    fn test_query_filter_key_prefix() {
        let filter = QueryFilter::KeyPrefix(b"prefix".to_vec());
        assert!(filter.matches(None, 0, b"prefix-key"));
        assert!(filter.matches(None, 0, b"prefix"));
        assert!(!filter.matches(None, 0, b"other-key"));
    }

    #[test]
    fn test_query_filter_and() {
        let filter = QueryFilter::And(vec![
            QueryFilter::BlockAfter(50),
            QueryFilter::BlockBefore(150),
        ]);
        assert!(!filter.matches(Some(50), 0, b"key"));
        assert!(filter.matches(Some(100), 0, b"key"));
        assert!(!filter.matches(Some(150), 0, b"key"));
    }

    #[test]
    fn test_query_filter_or() {
        let filter = QueryFilter::Or(vec![
            QueryFilter::BlockBefore(50),
            QueryFilter::BlockAfter(150),
        ]);
        assert!(filter.matches(Some(25), 0, b"key"));
        assert!(!filter.matches(Some(100), 0, b"key"));
        assert!(filter.matches(Some(200), 0, b"key"));
    }

    #[test]
    fn test_query_builder_basic() {
        let builder = QueryBuilder::new("submissions")
            .before_block(1000)
            .limit(50);

        assert_eq!(builder.namespace(), "submissions");
        assert_eq!(builder.get_limit(), 50);
        assert!(builder.is_ascending());
    }

    #[test]
    fn test_query_builder_chaining() {
        let builder = QueryBuilder::new("submissions")
            .after_block(100)
            .before_block(1000)
            .limit(25)
            .descending()
            .with_count();

        assert!(!builder.is_ascending());
        assert!(builder.should_include_count());
        assert_eq!(builder.filters().len(), 2);
    }

    #[test]
    fn test_query_builder_build_filter() {
        let builder = QueryBuilder::new("test");
        assert!(builder.build_filter().is_none());

        let builder = QueryBuilder::new("test").before_block(100);
        let filter = builder.build_filter().expect("should have filter");
        assert!(matches!(filter, QueryFilter::BlockBefore(100)));

        let builder = QueryBuilder::new("test").after_block(50).before_block(150);
        let filter = builder.build_filter().expect("should have filter");
        assert!(matches!(filter, QueryFilter::And(_)));
    }

    #[test]
    fn test_block_index_key_generation() {
        let key = StorageKey::new("submissions", "test-key");
        let index_key = block_index_key("submissions", 1000, &key);

        // Should start with namespace
        assert!(index_key.starts_with(b"submissions:"));

        // Parse it back
        let (namespace, block_id, key_hash) =
            parse_block_index_key(&index_key).expect("should parse");
        assert_eq!(namespace, "submissions");
        assert_eq!(block_id, 1000);
        assert_eq!(key_hash, key.hash());
    }

    #[test]
    fn test_block_index_key_ordering() {
        let key1 = StorageKey::new("submissions", "key1");
        let key2 = StorageKey::new("submissions", "key2");

        let idx1 = block_index_key("submissions", 100, &key1);
        let idx2 = block_index_key("submissions", 200, &key2);
        let idx3 = block_index_key("submissions", 100, &key2);

        // Block 100 should come before block 200
        assert!(idx1 < idx2);
        assert!(idx3 < idx2);
    }

    #[test]
    fn test_block_range_keys() {
        let start = block_range_start("submissions", 100);
        let end = block_range_end("submissions", 200);

        assert!(start < end);

        // Keys within range should be between start and end
        let key = StorageKey::new("submissions", "test");
        let idx_150 = block_index_key("submissions", 150, &key);
        assert!(idx_150 > start);
        assert!(idx_150 < end);

        // Keys outside range
        let idx_50 = block_index_key("submissions", 50, &key);
        assert!(idx_50 < start);

        let idx_250 = block_index_key("submissions", 250, &key);
        assert!(idx_250 > end);
    }

    #[test]
    fn test_block_range_end_overflow() {
        let end = block_range_end("test", u64::MAX);
        // Should not panic, uses saturating_add
        assert!(!end.is_empty());
    }

    #[test]
    fn test_parse_block_index_key_invalid() {
        // Too short
        assert!(parse_block_index_key(b"short").is_none());

        // No colon
        assert!(parse_block_index_key(b"nonamespace").is_none());

        // Invalid structure
        let mut bad_key = b"ns:".to_vec();
        bad_key.extend_from_slice(&[0u8; 8]); // block_id
        bad_key.push(b'X'); // wrong separator
        bad_key.extend_from_slice(&[0u8; 32]); // hash
        assert!(parse_block_index_key(&bad_key).is_none());
    }
}
