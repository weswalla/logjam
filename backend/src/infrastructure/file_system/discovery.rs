/// File discovery utilities for finding Logseq markdown files
use std::path::{Path, PathBuf};
use tokio::fs;

/// Discover all .md files in a directory recursively
pub async fn discover_markdown_files(dir: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    Box::pin(async move {
        let mut files = Vec::new();
        let mut entries = fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_file() {
                if let Some(extension) = path.extension() {
                    if extension == "md" {
                        files.push(path);
                    }
                }
            } else if path.is_dir() {
                // Skip hidden directories and logseq internal directories
                if let Some(dir_name) = path.file_name().and_then(|n| n.to_str()) {
                    if !dir_name.starts_with('.') && dir_name != "logseq" {
                        let mut sub_files = discover_markdown_files(&path).await?;
                        files.append(&mut sub_files);
                    }
                }
            }
        }

        Ok(files)
    }).await
}

/// Discover markdown files in both pages/ and journals/ subdirectories
pub async fn discover_logseq_files(logseq_dir: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut all_files = Vec::new();

    // Discover files in pages/
    let pages_dir = logseq_dir.join("pages");
    if pages_dir.exists() {
        let mut pages_files = discover_markdown_files(&pages_dir).await?;
        all_files.append(&mut pages_files);
    }

    // Discover files in journals/
    let journals_dir = logseq_dir.join("journals");
    if journals_dir.exists() {
        let mut journals_files = discover_markdown_files(&journals_dir).await?;
        all_files.append(&mut journals_files);
    }

    Ok(all_files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_discover_markdown_files() {
        // Create a temporary directory structure
        let temp_dir = TempDir::new().unwrap();
        let test_dir = temp_dir.path();

        // Create some markdown files
        fs::write(test_dir.join("file1.md"), "content").unwrap();
        fs::write(test_dir.join("file2.md"), "content").unwrap();
        fs::write(test_dir.join("file.txt"), "content").unwrap(); // Should be ignored

        // Create a subdirectory with more markdown files
        let sub_dir = test_dir.join("subdir");
        fs::create_dir(&sub_dir).unwrap();
        fs::write(sub_dir.join("file3.md"), "content").unwrap();

        let files = discover_markdown_files(test_dir).await.unwrap();

        assert_eq!(files.len(), 3); // Only .md files
    }

    #[tokio::test]
    async fn test_discover_logseq_files() {
        // Create a temporary Logseq directory structure
        let temp_dir = TempDir::new().unwrap();
        let logseq_dir = temp_dir.path();

        // Create pages/ and journals/ directories
        let pages_dir = logseq_dir.join("pages");
        let journals_dir = logseq_dir.join("journals");
        fs::create_dir(&pages_dir).unwrap();
        fs::create_dir(&journals_dir).unwrap();

        // Add files
        fs::write(pages_dir.join("page1.md"), "content").unwrap();
        fs::write(pages_dir.join("page2.md"), "content").unwrap();
        fs::write(journals_dir.join("2025_10_11.md"), "content").unwrap();

        let files = discover_logseq_files(logseq_dir).await.unwrap();

        assert_eq!(files.len(), 3);
    }
}
