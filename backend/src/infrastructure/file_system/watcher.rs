/// File system watcher using the notify crate
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebounceEventResult, Debouncer, DebouncedEventKind};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WatcherError {
    #[error("Notify error: {0}")]
    Notify(#[from] notify::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Simplified file event representation
#[derive(Debug, Clone)]
pub struct FileEvent {
    pub path: PathBuf,
    pub kind: FileEventKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileEventKind {
    Created,
    Modified,
    Deleted,
}

impl FileEvent {
    /// Check if this event is for a markdown file
    pub fn is_markdown(&self) -> bool {
        self.path
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext == "md")
            .unwrap_or(false)
    }

    /// Check if this event is in pages/ or journals/ directories
    pub fn is_in_logseq_dirs(&self) -> bool {
        self.path
            .ancestors()
            .any(|ancestor| {
                ancestor
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| name == "pages" || name == "journals")
                    .unwrap_or(false)
            })
    }
}

/// File watcher with debouncing for Logseq directories
pub struct LogseqFileWatcher {
    _debouncer: Debouncer<RecommendedWatcher>,
    receiver: Receiver<DebounceEventResult>,
}

impl LogseqFileWatcher {
    /// Create a new file watcher for the given directory
    /// Debounce duration is typically 500ms to handle rapid file changes
    pub fn new(
        path: &Path,
        debounce_duration: Duration,
    ) -> Result<Self, WatcherError> {
        let (tx, rx) = std::sync::mpsc::channel();

        let mut debouncer = new_debouncer(debounce_duration, tx)?;

        // Watch the directory recursively
        debouncer
            .watcher()
            .watch(path, RecursiveMode::Recursive)?;

        Ok(LogseqFileWatcher {
            _debouncer: debouncer,
            receiver: rx,
        })
    }

    /// Get the next batch of file events (non-blocking)
    pub fn try_recv(&self) -> Option<Vec<FileEvent>> {
        match self.receiver.try_recv() {
            Ok(Ok(events)) => {
                let file_events: Vec<FileEvent> = events
                    .into_iter()
                    .filter_map(|event| Self::convert_event(event.path, event.kind))
                    .collect();

                if file_events.is_empty() {
                    None
                } else {
                    Some(file_events)
                }
            }
            Ok(Err(errors)) => {
                tracing::error!("File watcher errors: {:?}", errors);
                None
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => None,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                tracing::error!("File watcher disconnected");
                None
            }
        }
    }

    /// Wait for the next batch of file events (blocking)
    pub fn recv(&self) -> Option<Vec<FileEvent>> {
        match self.receiver.recv() {
            Ok(Ok(events)) => {
                let file_events: Vec<FileEvent> = events
                    .into_iter()
                    .filter_map(|event| Self::convert_event(event.path, event.kind))
                    .collect();

                if file_events.is_empty() {
                    None
                } else {
                    Some(file_events)
                }
            }
            Ok(Err(errors)) => {
                tracing::error!("File watcher errors: {:?}", errors);
                None
            }
            Err(_) => {
                tracing::error!("File watcher disconnected");
                None
            }
        }
    }

    /// Convert a notify event to our simplified FileEvent
    fn convert_event(path: PathBuf, kind: DebouncedEventKind) -> Option<FileEvent> {
        let event_kind = match kind {
            DebouncedEventKind::Any => {
                // For debounced events, we treat "Any" as a modification
                // since it represents a file that changed in some way
                FileEventKind::Modified
            }
            _ => {
                // Handle any future variants of DebouncedEventKind
                FileEventKind::Modified
            }
        };

        let event = FileEvent { path, kind: event_kind };

        // Only return events for markdown files in pages/ or journals/
        if event.is_markdown() && event.is_in_logseq_dirs() {
            Some(event)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_event_is_markdown() {
        let event = FileEvent {
            path: PathBuf::from("/test/file.md"),
            kind: FileEventKind::Created,
        };
        assert!(event.is_markdown());

        let event2 = FileEvent {
            path: PathBuf::from("/test/file.txt"),
            kind: FileEventKind::Created,
        };
        assert!(!event2.is_markdown());
    }

    #[test]
    fn test_file_event_is_in_logseq_dirs() {
        let event = FileEvent {
            path: PathBuf::from("/logseq/pages/file.md"),
            kind: FileEventKind::Created,
        };
        assert!(event.is_in_logseq_dirs());

        let event2 = FileEvent {
            path: PathBuf::from("/logseq/journals/2025_10_11.md"),
            kind: FileEventKind::Created,
        };
        assert!(event2.is_in_logseq_dirs());

        let event3 = FileEvent {
            path: PathBuf::from("/logseq/assets/image.png"),
            kind: FileEventKind::Created,
        };
        assert!(!event3.is_in_logseq_dirs());
    }
}
