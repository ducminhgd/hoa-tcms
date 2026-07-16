//! Pagination utilities for list endpoints.

/// Default page size when none is specified.
pub const DEFAULT_PAGE_SIZE: u32 = 25;

/// Maximum allowed page size.
pub const MAX_PAGE_SIZE: u32 = 100;

/// Supported page sizes.
pub const PAGE_SIZE_OPTIONS: &[u32] = &[10, 25, 50, 100];

/// Pagination parameters parsed from query string.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
pub struct PaginationParams {
    #[serde(default)]
    pub page: u32,
    #[serde(default = "default_page_size")]
    pub limit: u32,
}

fn default_page_size() -> u32 {
    DEFAULT_PAGE_SIZE
}

impl PaginationParams {
    /// Validate and normalise pagination parameters.
    ///
    /// Returns `(offset, limit)` suitable for SQL `OFFSET` / `LIMIT`.
    pub fn to_offset_limit(&self) -> (u64, u64) {
        let page = self.page.max(1);
        let limit = self.limit.clamp(1, MAX_PAGE_SIZE);
        let offset = u64::from((page - 1) * limit);
        (offset, u64::from(limit))
    }

    /// Return the page number (1-based).
    pub fn page(&self) -> u32 {
        self.page.max(1)
    }

    /// Return the limit, capped to max.
    pub fn limit(&self) -> u32 {
        self.limit.clamp(1, MAX_PAGE_SIZE)
    }
}

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: 1,
            limit: DEFAULT_PAGE_SIZE,
        }
    }
}

/// Paginated response wrapper.
#[derive(Debug, serde::Serialize)]
pub struct PaginatedResponse<T: serde::Serialize> {
    pub data: Vec<T>,
    pub meta: PaginationMeta,
}

/// Pagination metadata embedded in list responses.
#[derive(Debug, serde::Serialize)]
pub struct PaginationMeta {
    pub total: u64,
    pub page: u32,
    pub limit: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

impl<T: serde::Serialize> PaginatedResponse<T> {
    /// Build a paginated response from a full dataset slice.
    pub fn new(data: Vec<T>, total: u64, params: &PaginationParams) -> Self {
        Self {
            data,
            meta: PaginationMeta {
                total,
                page: params.page(),
                limit: params.limit(),
                next_cursor: None,
            },
        }
    }

    /// Build a paginated response with a cursor for cursor-based pagination.
    pub fn with_cursor(
        data: Vec<T>,
        total: u64,
        params: &PaginationParams,
        cursor: String,
    ) -> Self {
        Self {
            data,
            meta: PaginationMeta {
                total,
                page: params.page(),
                limit: params.limit(),
                next_cursor: Some(cursor),
            },
        }
    }
}
