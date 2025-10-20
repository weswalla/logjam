/// Text preprocessing for semantic search embeddings
use regex::Regex;
use std::sync::OnceLock;

/// Text preprocessor that cleans Logseq syntax while preserving context
#[derive(Debug)]
pub struct TextPreprocessor {
    page_ref_regex: Regex,
    tag_regex: Regex,
    todo_regex: Regex,
}

impl TextPreprocessor {
    pub fn new() -> Self {
        TextPreprocessor {
            // Matches [[page reference]] patterns
            page_ref_regex: Regex::new(r"\[\[([^\]]+)\]\]").unwrap(),
            // Matches #tag patterns (word boundaries to avoid matching URLs)
            tag_regex: Regex::new(r"#(\w+)").unwrap(),
            // Matches TODO/DONE/LATER/NOW markers at the start
            todo_regex: Regex::new(r"^(TODO|DONE|LATER|NOW|IN-PROGRESS)\s+").unwrap(),
        }
    }

    /// Get a singleton instance (for efficiency in batch processing)
    pub fn instance() -> &'static Self {
        static INSTANCE: OnceLock<TextPreprocessor> = OnceLock::new();
        INSTANCE.get_or_init(TextPreprocessor::new)
    }

    /// Preprocess a block's content for embedding
    /// Removes Logseq syntax but keeps semantic meaning
    pub fn preprocess(&self, content: &str, page_title: &str, hierarchy_path: &[String]) -> String {
        let mut text = content.to_string();

        // Remove TODO/DONE markers
        text = self.todo_regex.replace(&text, "").to_string();

        // Replace [[page references]] with just the page name
        text = self.page_ref_regex.replace_all(&text, "$1").to_string();

        // Replace #tags with just the tag name
        text = self.tag_regex.replace_all(&text, "$1").to_string();

        // Add context: page title and hierarchy
        let mut context_parts = vec![];

        // Add page title as context
        if !page_title.is_empty() {
            context_parts.push(format!("Page: {}", page_title));
        }

        // Add parent blocks as context (limit to last 2 for brevity)
        if !hierarchy_path.is_empty() {
            let parent_count = hierarchy_path.len().min(2);
            let relevant_parents = &hierarchy_path[hierarchy_path.len() - parent_count..];
            if !relevant_parents.is_empty() {
                context_parts.push(format!("Context: {}", relevant_parents.join(" > ")));
            }
        }

        // Combine context with content
        if !context_parts.is_empty() {
            format!("{}. {}", context_parts.join(". "), text.trim())
        } else {
            text.trim().to_string()
        }
    }

    /// Chunk text into smaller pieces if it exceeds max_tokens
    /// Uses a simple word-based approach with overlap
    pub fn chunk_text(
        &self,
        text: &str,
        max_words: usize,
        overlap_words: usize,
    ) -> Vec<String> {
        let words: Vec<&str> = text.split_whitespace().collect();

        if words.len() <= max_words {
            return vec![text.to_string()];
        }

        let mut chunks = Vec::new();
        let mut start = 0;

        while start < words.len() {
            let end = (start + max_words).min(words.len());
            let chunk = words[start..end].join(" ");
            chunks.push(chunk);

            // If this was the last chunk, break
            if end >= words.len() {
                break;
            }

            // Move start forward, accounting for overlap
            start = end - overlap_words;
        }

        chunks
    }
}

impl Default for TextPreprocessor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_page_references() {
        let preprocessor = TextPreprocessor::new();
        let text = "This is a note about [[machine learning]] and [[AI]]";
        let result = preprocessor.preprocess(text, "", &[]);
        assert!(result.contains("machine learning"));
        assert!(result.contains("AI"));
        assert!(!result.contains("[["));
        assert!(!result.contains("]]"));
    }

    #[test]
    fn test_remove_tags() {
        let preprocessor = TextPreprocessor::new();
        let text = "This note has #programming and #rust tags";
        let result = preprocessor.preprocess(text, "", &[]);
        assert!(result.contains("programming"));
        assert!(result.contains("rust"));
        // The # should be removed but the word kept
        assert_eq!(result, "This note has programming and rust tags");
    }

    #[test]
    fn test_remove_todo_markers() {
        let preprocessor = TextPreprocessor::new();

        let todo_text = "TODO complete this task";
        let result = preprocessor.preprocess(todo_text, "", &[]);
        assert!(!result.contains("TODO"));
        assert!(result.contains("complete this task"));

        let done_text = "DONE completed task";
        let result2 = preprocessor.preprocess(done_text, "", &[]);
        assert!(!result2.contains("DONE"));
        assert!(result2.contains("completed task"));
    }

    #[test]
    fn test_add_page_title_context() {
        let preprocessor = TextPreprocessor::new();
        let text = "This is some content";
        let result = preprocessor.preprocess(text, "Programming Notes", &[]);
        assert!(result.contains("Page: Programming Notes"));
        assert!(result.contains("This is some content"));
    }

    #[test]
    fn test_add_hierarchy_context() {
        let preprocessor = TextPreprocessor::new();
        let text = "Nested content";
        let hierarchy = vec![
            "Parent block".to_string(),
            "Child block".to_string(),
            "Grandchild block".to_string(),
        ];
        let result = preprocessor.preprocess(text, "Page Title", &hierarchy);

        // Should only include last 2 parents
        assert!(result.contains("Context: Child block > Grandchild block"));
        assert!(!result.contains("Parent block"));
        assert!(result.contains("Nested content"));
    }

    #[test]
    fn test_full_preprocessing() {
        let preprocessor = TextPreprocessor::new();
        let text = "TODO Read [[Programming in Rust]] book about #async programming";
        let hierarchy = vec!["Learning Resources".to_string()];
        let result = preprocessor.preprocess(text, "Book Notes", &hierarchy);

        assert!(!result.contains("TODO"));
        assert!(!result.contains("[["));
        assert!(!result.contains("]]"));
        assert!(!result.contains("#async"));
        assert!(result.contains("Page: Book Notes"));
        assert!(result.contains("Context: Learning Resources"));
        assert!(result.contains("Programming in Rust"));
        assert!(result.contains("async programming"));
    }

    #[test]
    fn test_chunk_short_text() {
        let preprocessor = TextPreprocessor::new();
        let text = "This is a short text";
        let chunks = preprocessor.chunk_text(text, 10, 2);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], text);
    }

    #[test]
    fn test_chunk_long_text() {
        let preprocessor = TextPreprocessor::new();
        let text = "one two three four five six seven eight nine ten eleven twelve";
        let chunks = preprocessor.chunk_text(text, 5, 2);

        // Should create multiple chunks
        assert!(chunks.len() > 1);

        // First chunk should have 5 words
        assert_eq!(chunks[0], "one two three four five");

        // Second chunk should have overlap (last 2 words from first chunk)
        assert!(chunks[1].starts_with("four five"));
    }

    #[test]
    fn test_chunk_with_overlap() {
        let preprocessor = TextPreprocessor::new();
        let text = "a b c d e f g h i j";
        let chunks = preprocessor.chunk_text(text, 4, 1);

        assert_eq!(chunks[0], "a b c d");
        assert_eq!(chunks[1], "d e f g");
        assert_eq!(chunks[2], "g h i j");
    }
}
