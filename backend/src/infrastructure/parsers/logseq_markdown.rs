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
                    parent_id,
                    IndentLevel::new(indent_level),
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
        let mut position = 0;
        let chars: Vec<char> = content.chars().collect();

        while position < chars.len() {
            // Check for [[page reference]]
            if position + 1 < chars.len()
                && chars[position] == '['
                && chars[position + 1] == '[' {
                position += 2; // skip [[
                let mut ref_text = String::new();

                // Find closing ]]
                while position + 1 < chars.len() {
                    if chars[position] == ']' && chars[position + 1] == ']' {
                        position += 2; // skip ]]
                        if !ref_text.is_empty() {
                            if let Ok(page_ref) = PageReference::from_brackets(&ref_text) {
                                references.push(page_ref);
                            }
                        }
                        break;
                    } else {
                        ref_text.push(chars[position]);
                        position += 1;
                    }
                }
            }
            // Check for #tag
            else if chars[position] == '#' {
                // Make sure it's at word boundary (start of string or after whitespace)
                let at_word_boundary = position == 0 || chars[position - 1].is_whitespace();

                if at_word_boundary && position + 1 < chars.len() {
                    position += 1; // skip #
                    let mut tag = String::new();

                    // Collect tag characters (until whitespace or punctuation)
                    while position < chars.len()
                        && !chars[position].is_whitespace()
                        && !chars[position].is_ascii_punctuation() {
                        tag.push(chars[position]);
                        position += 1;
                    }

                    if !tag.is_empty() {
                        if let Ok(tag_ref) = PageReference::from_tag(&tag) {
                            references.push(tag_ref);
                        }
                    }
                } else {
                    position += 1;
                }
            } else {
                position += 1;
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

        let all_blocks: Vec<_> = page.all_blocks().collect();
        assert_eq!(all_blocks.len(), 3);

        // First block should have a URL
        let block1 = all_blocks[0];
        assert_eq!(block1.urls().len(), 1);

        // Second block should have a page reference
        let block2 = all_blocks[1];
        assert_eq!(block2.page_references().len(), 1);

        // Third block should have a tag
        let block3 = all_blocks[2];
        assert_eq!(block3.page_references().len(), 1);
    }
}
