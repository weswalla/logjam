/// Value objects for the domain layer
use super::base::{DomainError, DomainResult, ValueObject};
use std::fmt;
use std::path::{Path, PathBuf};

/// Unique identifier for a Page
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PageId(String);

impl PageId {
    pub fn new(id: impl Into<String>) -> DomainResult<Self> {
        let id = id.into();
        if id.is_empty() {
            return Err(DomainError::InvalidValue("PageId cannot be empty".to_string()));
        }
        Ok(PageId(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for PageId {}

impl fmt::Display for PageId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Unique identifier for a Block
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BlockId(String);

impl BlockId {
    pub fn new(id: impl Into<String>) -> DomainResult<Self> {
        let id = id.into();
        if id.is_empty() {
            return Err(DomainError::InvalidValue("BlockId cannot be empty".to_string()));
        }
        Ok(BlockId(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl ValueObject for BlockId {}

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A URL value object
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Url {
    value: String,
}

impl Url {
    pub fn new(url: impl Into<String>) -> DomainResult<Self> {
        let url = url.into();
        if url.is_empty() {
            return Err(DomainError::InvalidValue("URL cannot be empty".to_string()));
        }

        // Basic URL validation - should start with http:// or https://
        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(DomainError::InvalidValue(
                "URL must start with http:// or https://".to_string(),
            ));
        }

        Ok(Url { value: url })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    /// Get the domain from the URL
    pub fn domain(&self) -> Option<String> {
        // Simple extraction - in production, use a proper URL parser
        self.value
            .split("://")
            .nth(1)?
            .split('/')
            .next()
            .map(|s| s.to_string())
    }
}

impl ValueObject for Url {}

impl fmt::Display for Url {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}

/// A reference to another page (e.g., [[page-name]] or #tag)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PageReference {
    /// The title/name of the referenced page
    title: String,
    /// Whether this is a tag (starts with #) or a page reference (surrounded by [[]])
    is_tag: bool,
}

impl PageReference {
    /// Create a page reference from [[page-name]] format
    pub fn from_brackets(title: impl Into<String>) -> DomainResult<Self> {
        let title = title.into();
        if title.is_empty() {
            return Err(DomainError::InvalidValue(
                "Page reference title cannot be empty".to_string(),
            ));
        }
        Ok(PageReference {
            title,
            is_tag: false,
        })
    }

    /// Create a tag reference from #tag format
    pub fn from_tag(title: impl Into<String>) -> DomainResult<Self> {
        let title = title.into();
        if title.is_empty() {
            return Err(DomainError::InvalidValue("Tag cannot be empty".to_string()));
        }
        Ok(PageReference {
            title,
            is_tag: true,
        })
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn is_tag(&self) -> bool {
        self.is_tag
    }

    pub fn is_page_reference(&self) -> bool {
        !self.is_tag
    }
}

impl ValueObject for PageReference {}

impl fmt::Display for PageReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_tag {
            write!(f, "#{}", self.title)
        } else {
            write!(f, "[[{}]]", self.title)
        }
    }
}

/// The content of a block as plain text
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockContent {
    text: String,
}

impl BlockContent {
    pub fn new(text: impl Into<String>) -> Self {
        BlockContent { text: text.into() }
    }

    pub fn as_str(&self) -> &str {
        &self.text
    }

    pub fn is_empty(&self) -> bool {
        self.text.trim().is_empty()
    }
}

impl ValueObject for BlockContent {}

impl fmt::Display for BlockContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.text)
    }
}

/// The indentation level of a block (0 = root level, 1 = first indent, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IndentLevel(usize);

impl IndentLevel {
    pub fn root() -> Self {
        IndentLevel(0)
    }

    pub fn new(level: usize) -> Self {
        IndentLevel(level)
    }

    pub fn value(&self) -> usize {
        self.0
    }

    pub fn increment(&self) -> Self {
        IndentLevel(self.0 + 1)
    }

    pub fn decrement(&self) -> Option<Self> {
        if self.0 > 0 {
            Some(IndentLevel(self.0 - 1))
        } else {
            None
        }
    }
}

impl ValueObject for IndentLevel {}

impl fmt::Display for IndentLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A validated Logseq directory path that contains pages/ and journals/ subdirectories
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogseqDirectoryPath {
    path: PathBuf,
}

impl LogseqDirectoryPath {
    pub fn new(path: impl Into<PathBuf>) -> DomainResult<Self> {
        let path = path.into();

        // Validate that the path exists and is a directory
        if !path.exists() {
            return Err(DomainError::InvalidValue(format!(
                "Directory does not exist: {}",
                path.display()
            )));
        }

        if !path.is_dir() {
            return Err(DomainError::InvalidValue(format!(
                "Path is not a directory: {}",
                path.display()
            )));
        }

        // Validate that pages/ and journals/ subdirectories exist
        let pages_dir = path.join("pages");
        let journals_dir = path.join("journals");

        if !pages_dir.exists() || !pages_dir.is_dir() {
            return Err(DomainError::InvalidValue(format!(
                "Directory does not contain a 'pages' subdirectory: {}",
                path.display()
            )));
        }

        if !journals_dir.exists() || !journals_dir.is_dir() {
            return Err(DomainError::InvalidValue(format!(
                "Directory does not contain a 'journals' subdirectory: {}",
                path.display()
            )));
        }

        Ok(LogseqDirectoryPath { path })
    }

    pub fn as_path(&self) -> &Path {
        &self.path
    }

    pub fn pages_dir(&self) -> PathBuf {
        self.path.join("pages")
    }

    pub fn journals_dir(&self) -> PathBuf {
        self.path.join("journals")
    }
}

impl ValueObject for LogseqDirectoryPath {}

impl fmt::Display for LogseqDirectoryPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path.display())
    }
}

/// Tracks the progress of an import operation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportProgress {
    files_processed: usize,
    total_files: usize,
    current_file: Option<PathBuf>,
}

impl ImportProgress {
    pub fn new(total_files: usize) -> Self {
        ImportProgress {
            files_processed: 0,
            total_files,
            current_file: None,
        }
    }

    pub fn increment(&mut self) {
        self.files_processed += 1;
    }

    pub fn set_current_file(&mut self, file: Option<PathBuf>) {
        self.current_file = file;
    }

    pub fn files_processed(&self) -> usize {
        self.files_processed
    }

    pub fn total_files(&self) -> usize {
        self.total_files
    }

    pub fn current_file(&self) -> Option<&PathBuf> {
        self.current_file.as_ref()
    }

    pub fn percentage(&self) -> f64 {
        if self.total_files == 0 {
            return 100.0;
        }
        (self.files_processed as f64 / self.total_files as f64) * 100.0
    }
}

impl ValueObject for ImportProgress {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_id_creation() {
        let id = PageId::new("test-page").unwrap();
        assert_eq!(id.as_str(), "test-page");

        let empty_id = PageId::new("");
        assert!(empty_id.is_err());
    }

    #[test]
    fn test_block_id_creation() {
        let id = BlockId::new("block-123").unwrap();
        assert_eq!(id.as_str(), "block-123");

        let empty_id = BlockId::new("");
        assert!(empty_id.is_err());
    }

    #[test]
    fn test_url_creation() {
        let url = Url::new("https://example.com").unwrap();
        assert_eq!(url.as_str(), "https://example.com");

        let invalid_url = Url::new("not-a-url");
        assert!(invalid_url.is_err());

        let empty_url = Url::new("");
        assert!(empty_url.is_err());
    }

    #[test]
    fn test_url_domain_extraction() {
        let url = Url::new("https://example.com/path/to/page").unwrap();
        assert_eq!(url.domain(), Some("example.com".to_string()));

        let url2 = Url::new("https://subdomain.example.com").unwrap();
        assert_eq!(url2.domain(), Some("subdomain.example.com".to_string()));
    }

    #[test]
    fn test_page_reference_creation() {
        let ref1 = PageReference::from_brackets("my-page").unwrap();
        assert_eq!(ref1.title(), "my-page");
        assert!(!ref1.is_tag());
        assert!(ref1.is_page_reference());
        assert_eq!(ref1.to_string(), "[[my-page]]");

        let ref2 = PageReference::from_tag("my-tag").unwrap();
        assert_eq!(ref2.title(), "my-tag");
        assert!(ref2.is_tag());
        assert!(!ref2.is_page_reference());
        assert_eq!(ref2.to_string(), "#my-tag");

        let empty_ref = PageReference::from_brackets("");
        assert!(empty_ref.is_err());
    }

    #[test]
    fn test_block_content() {
        let content = BlockContent::new("This is some text");
        assert_eq!(content.as_str(), "This is some text");
        assert!(!content.is_empty());

        let empty_content = BlockContent::new("   ");
        assert!(empty_content.is_empty());
    }

    #[test]
    fn test_indent_level() {
        let root = IndentLevel::root();
        assert_eq!(root.value(), 0);

        let level1 = root.increment();
        assert_eq!(level1.value(), 1);

        let level2 = level1.increment();
        assert_eq!(level2.value(), 2);

        let back_to_1 = level2.decrement().unwrap();
        assert_eq!(back_to_1.value(), 1);

        let back_to_0 = back_to_1.decrement().unwrap();
        assert_eq!(back_to_0.value(), 0);

        let none = back_to_0.decrement();
        assert!(none.is_none());
    }

    #[test]
    fn test_logseq_directory_path() {
        // Test that a non-directory path fails validation
        let invalid_path = LogseqDirectoryPath::new("/non/existent/path");
        assert!(invalid_path.is_err());
    }

    #[test]
    fn test_import_progress() {
        let mut progress = ImportProgress::new(10);
        assert_eq!(progress.files_processed(), 0);
        assert_eq!(progress.total_files(), 10);
        assert_eq!(progress.percentage(), 0.0);
        assert!(progress.current_file().is_none());

        progress.increment();
        assert_eq!(progress.files_processed(), 1);
        assert_eq!(progress.percentage(), 10.0);

        progress.set_current_file(Some(PathBuf::from("/test/file.md")));
        assert_eq!(progress.current_file().unwrap().to_str().unwrap(), "/test/file.md");

        for _ in 0..9 {
            progress.increment();
        }
        assert_eq!(progress.files_processed(), 10);
        assert_eq!(progress.percentage(), 100.0);
    }
}
