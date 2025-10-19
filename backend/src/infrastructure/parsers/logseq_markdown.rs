/// Logseq markdown parser - converts .md files into Page and Block domain objects
use crate::domain::aggregates::Page;
use crate::domain::entities::Block;
use crate::domain::value_objects::{
    BlockContent, BlockId, IndentLevel, PageId, PageReference, Url,
};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid markdown structure: {0}")]
    InvalidMarkdown(String),

    #[error("Domain error: {0}")]
    Domain(#[from] crate::domain::base::DomainError),
}

pub type ParseResult<T> = Result<T, ParseError>;

/// Parser for Logseq markdown files
pub struct LogseqMarkdownParser;

impl LogseqMarkdownParser {
    /// Parse a markdown file from the given path
    pub async fn parse_file(path: &Path) -> ParseResult<Page> {
        let content = tokio::fs::read_to_string(path).await?;

        // Extract title from filename (without .md extension)
        let title = path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| ParseError::InvalidMarkdown("Invalid filename".to_string()))?
            .to_string();

        // Generate page ID from title (could be more sophisticated)
        let page_id = PageId::new(format!("page-{}", uuid::Uuid::new_v4()))?;

        Self::parse_content(&content, page_id, title)
    }

    /// Parse markdown content into a Page with Blocks
    pub fn parse_content(content: &str, page_id: PageId, title: String) -> ParseResult<Page> {
        let mut page = Page::new(page_id, title);

        // Parse lines into blocks
        let lines: Vec<&str> = content.lines().collect();
        let blocks = Self::parse_blocks(&lines)?;

        // Build the block hierarchy and add to page
        Self::build_hierarchy(&mut page, blocks)?;

        Ok(page)
    }

    /// Parse lines into blocks with indentation information
    fn parse_blocks(lines: &[&str]) -> ParseResult<Vec<(usize, String)>> {
        let mut blocks = Vec::new();

        for line in lines {
            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            // Count leading tabs or spaces (assuming tab or 2 spaces = 1 indent level)
            let indent_level = Self::calculate_indent_level(line);

            // Extract content (remove bullet point marker if present)
            let content = Self::extract_content(line);

            // Skip if content is empty after extraction
            if content.trim().is_empty() {
                continue;
            }

            blocks.push((indent_level, content));
        }

        Ok(blocks)
    }

    /// Calculate indentation level from leading whitespace
    fn calculate_indent_level(line: &str) -> usize {
        let mut indent = 0;
        let mut chars = line.chars();

        while let Some(ch) = chars.next() {
            match ch {
                '\t' => indent += 1,
                ' ' => {
                    // Count groups of 2 spaces as one indent level
                    if chars.next() == Some(' ') {
                        indent += 1;
                    }
                }
                _ => break,
            }
        }

        indent
    }

    /// Extract content from a line, removing bullet markers
    fn extract_content(line: &str) -> String {
        let trimmed = line.trim_start();

        // Remove common bullet point markers: -, *, +
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ ") {
            trimmed[2..].to_string()
        } else if trimmed.starts_with('-') || trimmed.starts_with('*') || trimmed.starts_with('+') {
            trimmed[1..].trim_start().to_string()
        } else {
            trimmed.to_string()
        }
    }

    /// Build block hierarchy and add blocks to the page
    fn build_hierarchy(page: &mut Page, blocks: Vec<(usize, String)>) -> ParseResult<()> {
        // Track the parent block at each indent level
        let mut parent_stack: HashMap<usize, BlockId> = HashMap::new();

        for (indent_level, content) in blocks {
            // Generate unique block ID
            let block_id = BlockId::new(format!("block-{}", uuid::Uuid::new_v4()))?;

            // Extract URLs and page references from content
            let urls = Self::extract_urls(&content);
            let page_refs = Self::extract_page_references(&content);

            // Create block
            let mut block = if indent_level == 0 {
                Block::new_root(
                    block_id.clone(),
                    BlockContent::new(content),
                    IndentLevel::root(),
                )
            } else {
                // Find parent block at previous indent level
                let parent_id = parent_stack
                    .get(&(indent_level - 1))
                    .ok_or_else(|| {
                        ParseError::InvalidMarkdown(format!(
                            "No parent block found for indent level {}",
                            indent_level
                        ))
                    })?
                    .clone();

                Block::new_child(
                    block_id.clone(),
                    BlockContent::new(content),
                    IndentLevel::new(indent_level),
                    parent_id,
                )
            };

            // Add URLs and page references to block
            for url in urls {
                block.add_url(url);
            }
            for page_ref in page_refs {
                block.add_page_reference(page_ref);
            }

            // Add block to page
            page.add_block(block)?;

            // Update parent stack for this indent level
            parent_stack.insert(indent_level, block_id);

            // Clear deeper indent levels from stack
            parent_stack.retain(|level, _| *level <= indent_level);
        }

        Ok(())
    }

    /// Extract URLs from content (http:// and https://)
    fn extract_urls(content: &str) -> Vec<Url> {
        let mut urls = Vec::new();

        // Simple regex-like extraction (in production, use a proper URL parser)
        let words: Vec<&str> = content.split_whitespace().collect();

        for word in words {
            // Remove trailing punctuation
            let cleaned = word.trim_end_matches(|c: char| c.is_ascii_punctuation());

            if cleaned.starts_with("http://") || cleaned.starts_with("https://") {
                if let Ok(url) = Url::new(cleaned) {
                    urls.push(url);
                }
            }
        }

        urls
    }

    /// Extract page references from content ([[page]] and #tag)
    fn extract_page_references(content: &str) -> Vec<PageReference> {
        let mut references = Vec::new();

        // Extract [[page references]]
        let mut chars = content.chars().peekable();
        let mut current_ref = String::new();
        let mut in_brackets = false;
        let mut bracket_count = 0;

        while let Some(ch) = chars.next() {
            if ch == '[' && chars.peek() == Some(&'[') {
                chars.next(); // consume second [
                in_brackets = true;
                bracket_count = 2;
                current_ref.clear();
            } else if in_brackets && ch == ']' && chars.peek() == Some(&']') {
                chars.next(); // consume second ]
                if !current_ref.is_empty() {
                    if let Ok(page_ref) = PageReference::from_brackets(&current_ref) {
                        references.push(page_ref);
                    }
                }
                in_brackets = false;
                current_ref.clear();
            } else if in_brackets {
                current_ref.push(ch);
            }
        }

        // Extract #tags
        for word in content.split_whitespace() {
            if word.starts_with('#') && word.len() > 1 {
                let tag = word[1..].trim_end_matches(|c: char| c.is_ascii_punctuation());
                if !tag.is_empty() {
                    if let Ok(tag_ref) = PageReference::from_tag(tag) {
                        references.push(tag_ref);
                    }
                }
            }
        }

        references
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_indent_level() {
        assert_eq!(LogseqMarkdownParser::calculate_indent_level("- Text"), 0);
        assert_eq!(LogseqMarkdownParser::calculate_indent_level("\t- Text"), 1);
        assert_eq!(LogseqMarkdownParser::calculate_indent_level("  - Text"), 1);
        assert_eq!(LogseqMarkdownParser::calculate_indent_level("\t\t- Text"), 2);
        assert_eq!(LogseqMarkdownParser::calculate_indent_level("    - Text"), 2);
    }

    #[test]
    fn test_extract_content() {
        assert_eq!(LogseqMarkdownParser::extract_content("- Text"), "Text");
        assert_eq!(LogseqMarkdownParser::extract_content("* Text"), "Text");
        assert_eq!(LogseqMarkdownParser::extract_content("+ Text"), "Text");
        assert_eq!(LogseqMarkdownParser::extract_content("  - Text"), "Text");
        assert_eq!(LogseqMarkdownParser::extract_content("Text without bullet"), "Text without bullet");
    }

    #[test]
    fn test_extract_urls() {
        let content = "Check out https://example.com and http://test.org for more info.";
        let urls = LogseqMarkdownParser::extract_urls(content);

        assert_eq!(urls.len(), 2);
        assert_eq!(urls[0].as_str(), "https://example.com");
        assert_eq!(urls[1].as_str(), "http://test.org");
    }

    #[test]
    fn test_extract_page_references() {
        let content = "This mentions [[page name]] and #tag and [[another page]]";
        let refs = LogseqMarkdownParser::extract_page_references(content);

        assert_eq!(refs.len(), 3);
        assert_eq!(refs[0].title(), "page name");
        assert!(!refs[0].is_tag());
        assert_eq!(refs[1].title(), "tag");
        assert!(refs[1].is_tag());
        assert_eq!(refs[2].title(), "another page");
        assert!(!refs[2].is_tag());
    }

    #[test]
    fn test_parse_simple_markdown() {
        let content = "- First block\n- Second block\n  - Nested block\n- Third block";
        let page_id = PageId::new("test-page").unwrap();

        let page = LogseqMarkdownParser::parse_content(content, page_id, "Test Page".to_string()).unwrap();

        assert_eq!(page.title(), "Test Page");
        assert_eq!(page.root_blocks().len(), 3); // Three root-level blocks
    }

    #[test]
    fn test_parse_with_urls_and_references() {
        let content = "- Check https://example.com\n- See [[related page]] for more\n- Don't forget #tag";
        let page_id = PageId::new("test-page").unwrap();

        let page = LogseqMarkdownParser::parse_content(content, page_id, "Test Page".to_string()).unwrap();

        let all_blocks = page.all_blocks();
        assert_eq!(all_blocks.len(), 3);

        // First block should have a URL
        let block1 = all_blocks.iter().next().unwrap();
        assert_eq!(block1.urls().len(), 1);

        // Second block should have a page reference
        let block2 = all_blocks.iter().nth(1).unwrap();
        assert_eq!(block2.page_references().len(), 1);

        // Third block should have a tag
        let block3 = all_blocks.iter().nth(2).unwrap();
        assert_eq!(block3.page_references().len(), 1);
    }
}
