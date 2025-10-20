# Tauri Integration Implementation Plan

## Overview

Implement Tauri commands and event emitters to expose the Rust backend (ImportService, SyncService, PageRepository) to the frontend application. This bridges the gap between the domain/application layers and the UI layer following Tauri's IPC (Inter-Process Communication) patterns.

## Goals

1. **Expose backend services** via Tauri commands (async RPC)
2. **Stream real-time events** from services to frontend (progress, sync updates)
3. **Manage application state** (database connection, service lifecycle)
4. **Handle errors gracefully** with user-friendly error messages
5. **Support concurrent operations** (multiple frontend requests)
6. **Follow Tauri best practices** (state management, command patterns)

## Architecture Layer

**Presentation/API Layer** (`backend/src/tauri/`)

New layer hierarchy:
```
Frontend (React/Svelte/Vue)
    ↕ Tauri IPC
Backend API Layer (Tauri commands)
    ↕
Application Layer (Services, Repositories)
    ↕
Domain Layer (Entities, Aggregates)
    ↕
Infrastructure Layer (Persistence, File System)
```

## Dependencies

### Backend (Rust)

```toml
# backend/Cargo.toml
[dependencies]
tauri = { version = "2.3", features = ["protocol-asset"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.41", features = ["full"] }
```

### Frontend

```json
// frontend/package.json
{
  "dependencies": {
    "@tauri-apps/api": "^2.3.0",
    "@tauri-apps/plugin-shell": "^2.0.0"
  }
}
```

## Tauri State Management

### Application State

```rust
// backend/src/tauri/state.rs

use std::sync::Arc;
use tokio::sync::Mutex;
use crate::infrastructure::persistence::{SqlitePageRepository, SqliteFileMappingRepository};
use crate::application::services::{ImportService, SyncService};

/// Global application state shared across Tauri commands
pub struct AppState {
    pub page_repository: Arc<Mutex<SqlitePageRepository>>,
    pub mapping_repository: Arc<Mutex<SqliteFileMappingRepository>>,
    pub sync_service: Arc<Mutex<Option<SyncService<SqlitePageRepository, SqliteFileMappingRepository>>>>,
}

impl AppState {
    pub async fn new(db_path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let page_repo = SqlitePageRepository::new(db_path.as_ref()).await?;
        let mapping_repo = SqliteFileMappingRepository::new(page_repo.pool().clone()).await?;

        Ok(Self {
            page_repository: Arc::new(Mutex::new(page_repo)),
            mapping_repository: Arc::new(Mutex::new(mapping_repo)),
            sync_service: Arc::new(Mutex::new(None)),
        })
    }
}
```

### Tauri Builder Setup

```rust
// backend/src/main.rs or backend/src/lib.rs

use tauri::Manager;
use std::path::PathBuf;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            // Get app data directory
            let app_data_dir = app.path().app_data_dir()
                .expect("Failed to get app data directory");

            // Create data directory if it doesn't exist
            std::fs::create_dir_all(&app_data_dir)?;

            // Initialize database
            let db_path = app_data_dir.join("logjam.db");

            // Initialize app state
            let app_state = tauri::async_runtime::block_on(async {
                AppState::new(db_path).await
            })?;

            // Manage state (accessible in commands)
            app.manage(app_state);

            Ok(())
        })
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![
            // Import commands
            import_directory,
            get_import_status,

            // Sync commands
            start_sync,
            stop_sync,
            sync_once,
            get_sync_status,

            // Page query commands
            get_all_pages,
            get_page_by_id,
            get_page_by_title,
            delete_page,

            // Settings commands
            set_logseq_directory,
            get_logseq_directory,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## Data Transfer Objects (DTOs)

### Serializable DTOs for Frontend Communication

```rust
// backend/src/tauri/dto.rs

use serde::{Serialize, Deserialize};

// ============================================================================
// Import DTOs
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRequest {
    pub directory_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportSummaryDto {
    pub total_files: usize,
    pub successful: usize,
    pub failed: usize,
    pub duration_ms: u64,
    pub errors: Vec<ImportErrorDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportErrorDto {
    pub file_path: String,
    pub error_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ImportProgressEvent {
    Started { total_files: usize },
    FileProcessed { file_path: String, success: bool, current: usize, total: usize },
    Completed { summary: ImportSummaryDto },
    Failed { error: String },
}

// ============================================================================
// Sync DTOs
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncRequest {
    pub directory_path: String,
    pub enable_watch: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncSummaryDto {
    pub files_created: usize,
    pub files_updated: usize,
    pub files_deleted: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SyncEvent {
    Started,
    FileCreated { file_path: String },
    FileUpdated { file_path: String },
    FileDeleted { file_path: String },
    Completed { summary: SyncSummaryDto },
    Error { error: String },
}

// ============================================================================
// Page DTOs
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageDto {
    pub id: String,
    pub title: String,
    pub blocks: Vec<BlockDto>,
    pub root_block_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDto {
    pub id: String,
    pub content: String,
    pub indent_level: usize,
    pub parent_id: Option<String>,
    pub child_ids: Vec<String>,
    pub urls: Vec<UrlDto>,
    pub page_references: Vec<PageReferenceDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrlDto {
    pub url: String,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageReferenceDto {
    pub text: String,
    pub reference_type: String,  // "link" or "tag"
}

// ============================================================================
// Error DTOs
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub error_type: String,
}

impl ErrorResponse {
    pub fn new(error: impl std::fmt::Display, error_type: impl Into<String>) -> Self {
        Self {
            error: error.to_string(),
            error_type: error_type.into(),
        }
    }
}
```

### DTO Mappers

```rust
// backend/src/tauri/mappers.rs

use crate::domain::{Page, Block, Url, PageReference};
use crate::application::services::{ImportSummary, SyncSummary};
use super::dto::*;

pub struct DtoMapper;

impl DtoMapper {
    pub fn page_to_dto(page: &Page) -> PageDto {
        PageDto {
            id: page.id().as_str().to_string(),
            title: page.title().to_string(),
            blocks: page.all_blocks().map(Self::block_to_dto).collect(),
            root_block_ids: page.root_blocks().iter()
                .map(|id| id.as_str().to_string())
                .collect(),
        }
    }

    fn block_to_dto(block: &Block) -> BlockDto {
        BlockDto {
            id: block.id().as_str().to_string(),
            content: block.content().as_str().to_string(),
            indent_level: block.indent_level().level(),
            parent_id: block.parent_id().map(|id| id.as_str().to_string()),
            child_ids: block.child_ids().iter()
                .map(|id| id.as_str().to_string())
                .collect(),
            urls: block.urls().iter().map(Self::url_to_dto).collect(),
            page_references: block.page_references().iter()
                .map(Self::page_ref_to_dto)
                .collect(),
        }
    }

    fn url_to_dto(url: &Url) -> UrlDto {
        UrlDto {
            url: url.as_str().to_string(),
            domain: url.domain().map(String::from),
        }
    }

    fn page_ref_to_dto(page_ref: &PageReference) -> PageReferenceDto {
        PageReferenceDto {
            text: page_ref.text().to_string(),
            reference_type: match page_ref.reference_type() {
                ReferenceType::Link => "link",
                ReferenceType::Tag => "tag",
            }.to_string(),
        }
    }

    pub fn import_summary_to_dto(summary: &ImportSummary) -> ImportSummaryDto {
        ImportSummaryDto {
            total_files: summary.total_files,
            successful: summary.successful,
            failed: summary.failed,
            duration_ms: summary.duration.as_millis() as u64,
            errors: summary.errors.iter()
                .map(|(path, err)| ImportErrorDto {
                    file_path: path.to_string_lossy().to_string(),
                    error_message: err.to_string(),
                })
                .collect(),
        }
    }

    pub fn sync_summary_to_dto(summary: &SyncSummary) -> SyncSummaryDto {
        SyncSummaryDto {
            files_created: summary.files_created,
            files_updated: summary.files_updated,
            files_deleted: summary.files_deleted,
            duration_ms: summary.duration.as_millis() as u64,
        }
    }
}
```

## Tauri Commands

### Import Commands

```rust
// backend/src/tauri/commands/import.rs

use tauri::{AppHandle, State, Emitter};
use crate::tauri::{AppState, dto::*, mappers::DtoMapper};
use crate::application::services::ImportService;
use crate::domain::value_objects::LogseqDirectoryPath;

#[tauri::command]
pub async fn import_directory(
    app: AppHandle,
    state: State<'_, AppState>,
    request: ImportRequest,
) -> Result<ImportSummaryDto, ErrorResponse> {
    // Validate directory path
    let logseq_dir = LogseqDirectoryPath::new(&request.directory_path)
        .map_err(|e| ErrorResponse::new(e, "ValidationError"))?;

    // Create import service
    let page_repo = state.page_repository.lock().await.clone();
    let mapping_repo = state.mapping_repository.lock().await.clone();
    let mut import_service = ImportService::new(page_repo, mapping_repo);

    // Setup progress callback
    let app_clone = app.clone();
    let progress_callback = move |event: crate::domain::events::ImportProgressEvent| {
        let dto_event = match event {
            crate::domain::events::ImportProgressEvent::Started(progress) => {
                ImportProgressEvent::Started {
                    total_files: progress.total_files(),
                }
            }
            crate::domain::events::ImportProgressEvent::FileProcessed(progress) => {
                ImportProgressEvent::FileProcessed {
                    file_path: progress.current_file()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_default(),
                    success: true,
                    current: progress.processed_files(),
                    total: progress.total_files(),
                }
            }
        };

        // Emit event to frontend
        let _ = app_clone.emit("import-progress", dto_event);
    };

    // Run import
    let summary = import_service
        .import_directory(logseq_dir, Some(Arc::new(progress_callback)))
        .await
        .map_err(|e| ErrorResponse::new(e, "ImportError"))?;

    // Convert to DTO
    let summary_dto = DtoMapper::import_summary_to_dto(&summary);

    // Emit completion event
    app.emit("import-progress", ImportProgressEvent::Completed {
        summary: summary_dto.clone(),
    }).map_err(|e| ErrorResponse::new(e, "EventEmitError"))?;

    Ok(summary_dto)
}

#[tauri::command]
pub async fn get_import_status(
    state: State<'_, AppState>,
) -> Result<Option<ImportSummaryDto>, ErrorResponse> {
    // TODO: Implement persistent import status tracking
    // For now, return None (no active import)
    Ok(None)
}
```

### Sync Commands

```rust
// backend/src/tauri/commands/sync.rs

use tauri::{AppHandle, State, Emitter};
use crate::tauri::{AppState, dto::*, mappers::DtoMapper};
use crate::application::services::SyncService;
use crate::domain::value_objects::LogseqDirectoryPath;

#[tauri::command]
pub async fn start_sync(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SyncRequest,
) -> Result<(), ErrorResponse> {
    // Validate directory path
    let logseq_dir = LogseqDirectoryPath::new(&request.directory_path)
        .map_err(|e| ErrorResponse::new(e, "ValidationError"))?;

    // Check if sync is already running
    let sync_service_lock = state.sync_service.lock().await;
    if sync_service_lock.is_some() {
        return Err(ErrorResponse::new(
            "Sync is already running",
            "SyncAlreadyRunning",
        ));
    }
    drop(sync_service_lock);

    // Create sync service
    let sync_service = SyncService::new(
        state.page_repository.clone(),
        state.mapping_repository.clone(),
        logseq_dir,
    ).map_err(|e| ErrorResponse::new(e, "SyncError"))?;

    // Setup event callback
    let app_clone = app.clone();
    let sync_callback = move |event: crate::domain::events::SyncEvent| {
        let dto_event = match event {
            crate::domain::events::SyncEvent::Started => SyncEvent::Started,
            crate::domain::events::SyncEvent::FileCreated(path) => SyncEvent::FileCreated {
                file_path: path.to_string_lossy().to_string(),
            },
            crate::domain::events::SyncEvent::FileUpdated(path) => SyncEvent::FileUpdated {
                file_path: path.to_string_lossy().to_string(),
            },
            crate::domain::events::SyncEvent::FileDeleted(path) => SyncEvent::FileDeleted {
                file_path: path.to_string_lossy().to_string(),
            },
            crate::domain::events::SyncEvent::Completed(summary) => SyncEvent::Completed {
                summary: DtoMapper::sync_summary_to_dto(&summary),
            },
        };

        let _ = app_clone.emit("sync-event", dto_event);
    };

    // Store sync service in state
    let mut sync_service_lock = state.sync_service.lock().await;
    *sync_service_lock = Some(sync_service);
    drop(sync_service_lock);

    // Start watching in background task
    if request.enable_watch {
        let sync_service_clone = state.sync_service.clone();
        tauri::async_runtime::spawn(async move {
            let sync_service_lock = sync_service_clone.lock().await;
            if let Some(service) = sync_service_lock.as_ref() {
                let _ = service.start_watching(Some(Arc::new(sync_callback))).await;
            }
        });
    }

    Ok(())
}

#[tauri::command]
pub async fn stop_sync(
    state: State<'_, AppState>,
) -> Result<(), ErrorResponse> {
    let mut sync_service_lock = state.sync_service.lock().await;

    if sync_service_lock.is_none() {
        return Err(ErrorResponse::new(
            "No sync service running",
            "NoSyncRunning",
        ));
    }

    // Drop the sync service (stops watching)
    *sync_service_lock = None;

    Ok(())
}

#[tauri::command]
pub async fn sync_once(
    app: AppHandle,
    state: State<'_, AppState>,
    request: SyncRequest,
) -> Result<SyncSummaryDto, ErrorResponse> {
    // Validate directory path
    let logseq_dir = LogseqDirectoryPath::new(&request.directory_path)
        .map_err(|e| ErrorResponse::new(e, "ValidationError"))?;

    // Create temporary sync service
    let sync_service = SyncService::new(
        state.page_repository.clone(),
        state.mapping_repository.clone(),
        logseq_dir,
    ).map_err(|e| ErrorResponse::new(e, "SyncError"))?;

    // Setup callback
    let app_clone = app.clone();
    let callback = move |event: crate::domain::events::SyncEvent| {
        // Convert and emit events
        // (same as start_sync)
    };

    // Run one-time sync
    let summary = sync_service
        .sync_once(Some(Arc::new(callback)))
        .await
        .map_err(|e| ErrorResponse::new(e, "SyncError"))?;

    Ok(DtoMapper::sync_summary_to_dto(&summary))
}

#[tauri::command]
pub async fn get_sync_status(
    state: State<'_, AppState>,
) -> Result<bool, ErrorResponse> {
    let sync_service_lock = state.sync_service.lock().await;
    Ok(sync_service_lock.is_some())
}
```

### Page Query Commands

```rust
// backend/src/tauri/commands/pages.rs

use tauri::State;
use crate::tauri::{AppState, dto::*, mappers::DtoMapper};
use crate::domain::{PageId};
use crate::application::repositories::PageRepository;

#[tauri::command]
pub async fn get_all_pages(
    state: State<'_, AppState>,
) -> Result<Vec<PageDto>, ErrorResponse> {
    let page_repo = state.page_repository.lock().await;

    let pages = page_repo.find_all()
        .map_err(|e| ErrorResponse::new(e, "RepositoryError"))?;

    Ok(pages.iter().map(DtoMapper::page_to_dto).collect())
}

#[tauri::command]
pub async fn get_page_by_id(
    state: State<'_, AppState>,
    page_id: String,
) -> Result<Option<PageDto>, ErrorResponse> {
    let page_id = PageId::new(&page_id)
        .map_err(|e| ErrorResponse::new(e, "ValidationError"))?;

    let page_repo = state.page_repository.lock().await;

    let page = page_repo.find_by_id(&page_id)
        .map_err(|e| ErrorResponse::new(e, "RepositoryError"))?;

    Ok(page.as_ref().map(DtoMapper::page_to_dto))
}

#[tauri::command]
pub async fn get_page_by_title(
    state: State<'_, AppState>,
    title: String,
) -> Result<Option<PageDto>, ErrorResponse> {
    let page_repo = state.page_repository.lock().await;

    let page = page_repo.find_by_title(&title)
        .map_err(|e| ErrorResponse::new(e, "RepositoryError"))?;

    Ok(page.as_ref().map(DtoMapper::page_to_dto))
}

#[tauri::command]
pub async fn delete_page(
    state: State<'_, AppState>,
    page_id: String,
) -> Result<bool, ErrorResponse> {
    let page_id = PageId::new(&page_id)
        .map_err(|e| ErrorResponse::new(e, "ValidationError"))?;

    let mut page_repo = state.page_repository.lock().await;

    let deleted = page_repo.delete(&page_id)
        .map_err(|e| ErrorResponse::new(e, "RepositoryError"))?;

    Ok(deleted)
}
```

### Settings Commands

```rust
// backend/src/tauri/commands/settings.rs

use tauri::State;
use std::path::PathBuf;
use crate::tauri::{AppState, dto::ErrorResponse};

// For MVP, store in app state; later move to database
#[tauri::command]
pub async fn set_logseq_directory(
    _state: State<'_, AppState>,
    directory_path: String,
) -> Result<(), ErrorResponse> {
    // Validate directory exists and is a Logseq directory
    let path = PathBuf::from(&directory_path);

    if !path.exists() {
        return Err(ErrorResponse::new(
            "Directory does not exist",
            "DirectoryNotFound",
        ));
    }

    if !path.join("pages").exists() || !path.join("journals").exists() {
        return Err(ErrorResponse::new(
            "Not a valid Logseq directory (missing pages/ or journals/)",
            "InvalidLogseqDirectory",
        ));
    }

    // TODO: Persist to database or config file
    Ok(())
}

#[tauri::command]
pub async fn get_logseq_directory(
    _state: State<'_, AppState>,
) -> Result<Option<String>, ErrorResponse> {
    // TODO: Read from database or config file
    Ok(None)
}
```

## Frontend Integration

### TypeScript Types

```typescript
// frontend/src/types/tauri.ts

export interface ImportRequest {
  directory_path: string;
}

export interface ImportSummaryDto {
  total_files: number;
  successful: number;
  failed: number;
  duration_ms: number;
  errors: ImportErrorDto[];
}

export interface ImportErrorDto {
  file_path: string;
  error_message: string;
}

export type ImportProgressEvent =
  | { type: 'Started'; total_files: number }
  | { type: 'FileProcessed'; file_path: string; success: boolean; current: number; total: number }
  | { type: 'Completed'; summary: ImportSummaryDto }
  | { type: 'Failed'; error: string };

export interface SyncRequest {
  directory_path: string;
  enable_watch: boolean;
}

export interface SyncSummaryDto {
  files_created: number;
  files_updated: number;
  files_deleted: number;
  duration_ms: number;
}

export type SyncEvent =
  | { type: 'Started' }
  | { type: 'FileCreated'; file_path: string }
  | { type: 'FileUpdated'; file_path: string }
  | { type: 'FileDeleted'; file_path: string }
  | { type: 'Completed'; summary: SyncSummaryDto }
  | { type: 'Error'; error: string };

export interface PageDto {
  id: string;
  title: string;
  blocks: BlockDto[];
  root_block_ids: string[];
}

export interface BlockDto {
  id: string;
  content: string;
  indent_level: number;
  parent_id?: string;
  child_ids: string[];
  urls: UrlDto[];
  page_references: PageReferenceDto[];
}

export interface UrlDto {
  url: string;
  domain?: string;
}

export interface PageReferenceDto {
  text: string;
  reference_type: 'link' | 'tag';
}

export interface ErrorResponse {
  error: string;
  error_type: string;
}
```

### API Client

```typescript
// frontend/src/lib/tauri-api.ts

import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type {
  ImportRequest,
  ImportSummaryDto,
  ImportProgressEvent,
  SyncRequest,
  SyncSummaryDto,
  SyncEvent,
  PageDto,
  ErrorResponse,
} from '../types/tauri';

export class TauriApi {
  // ========== Import Commands ==========

  static async importDirectory(request: ImportRequest): Promise<ImportSummaryDto> {
    try {
      return await invoke<ImportSummaryDto>('import_directory', { request });
    } catch (error) {
      throw this.handleError(error);
    }
  }

  static async onImportProgress(callback: (event: ImportProgressEvent) => void) {
    return await listen<ImportProgressEvent>('import-progress', (event) => {
      callback(event.payload);
    });
  }

  // ========== Sync Commands ==========

  static async startSync(request: SyncRequest): Promise<void> {
    try {
      await invoke('start_sync', { request });
    } catch (error) {
      throw this.handleError(error);
    }
  }

  static async stopSync(): Promise<void> {
    try {
      await invoke('stop_sync');
    } catch (error) {
      throw this.handleError(error);
    }
  }

  static async syncOnce(request: SyncRequest): Promise<SyncSummaryDto> {
    try {
      return await invoke<SyncSummaryDto>('sync_once', { request });
    } catch (error) {
      throw this.handleError(error);
    }
  }

  static async getSyncStatus(): Promise<boolean> {
    try {
      return await invoke<boolean>('get_sync_status');
    } catch (error) {
      throw this.handleError(error);
    }
  }

  static async onSyncEvent(callback: (event: SyncEvent) => void) {
    return await listen<SyncEvent>('sync-event', (event) => {
      callback(event.payload);
    });
  }

  // ========== Page Commands ==========

  static async getAllPages(): Promise<PageDto[]> {
    try {
      return await invoke<PageDto[]>('get_all_pages');
    } catch (error) {
      throw this.handleError(error);
    }
  }

  static async getPageById(pageId: string): Promise<PageDto | null> {
    try {
      return await invoke<PageDto | null>('get_page_by_id', { pageId });
    } catch (error) {
      throw this.handleError(error);
    }
  }

  static async getPageByTitle(title: string): Promise<PageDto | null> {
    try {
      return await invoke<PageDto | null>('get_page_by_title', { title });
    } catch (error) {
      throw this.handleError(error);
    }
  }

  static async deletePage(pageId: string): Promise<boolean> {
    try {
      return await invoke<boolean>('delete_page', { pageId });
    } catch (error) {
      throw this.handleError(error);
    }
  }

  // ========== Settings Commands ==========

  static async setLogseqDirectory(directoryPath: string): Promise<void> {
    try {
      await invoke('set_logseq_directory', { directoryPath });
    } catch (error) {
      throw this.handleError(error);
    }
  }

  static async getLogseqDirectory(): Promise<string | null> {
    try {
      return await invoke<string | null>('get_logseq_directory');
    } catch (error) {
      throw this.handleError(error);
    }
  }

  // ========== Error Handling ==========

  private static handleError(error: unknown): Error {
    if (typeof error === 'object' && error !== null && 'error' in error) {
      const errorResponse = error as ErrorResponse;
      return new Error(`${errorResponse.error_type}: ${errorResponse.error}`);
    }
    return new Error(String(error));
  }
}
```

### React Hook Example

```typescript
// frontend/src/hooks/useImport.ts

import { useState, useEffect } from 'react';
import { TauriApi } from '../lib/tauri-api';
import type { ImportProgressEvent, ImportSummaryDto } from '../types/tauri';

export function useImport() {
  const [progress, setProgress] = useState<ImportProgressEvent | null>(null);
  const [isImporting, setIsImporting] = useState(false);

  useEffect(() => {
    const unlisten = TauriApi.onImportProgress((event) => {
      setProgress(event);

      if (event.type === 'Completed' || event.type === 'Failed') {
        setIsImporting(false);
      }
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const importDirectory = async (directoryPath: string) => {
    setIsImporting(true);
    setProgress({ type: 'Started', total_files: 0 });

    try {
      const summary = await TauriApi.importDirectory({ directory_path: directoryPath });
      return summary;
    } catch (error) {
      setProgress({ type: 'Failed', error: String(error) });
      throw error;
    }
  };

  return {
    progress,
    isImporting,
    importDirectory,
  };
}
```

```typescript
// frontend/src/hooks/useSync.ts

import { useState, useEffect } from 'react';
import { TauriApi } from '../lib/tauri-api';
import type { SyncEvent } from '../types/tauri';

export function useSync() {
  const [events, setEvents] = useState<SyncEvent[]>([]);
  const [isSyncing, setIsSyncing] = useState(false);

  useEffect(() => {
    const unlisten = TauriApi.onSyncEvent((event) => {
      setEvents((prev) => [...prev, event]);

      if (event.type === 'Started') {
        setIsSyncing(true);
      } else if (event.type === 'Completed' || event.type === 'Error') {
        setIsSyncing(false);
      }
    });

    // Check sync status on mount
    TauriApi.getSyncStatus().then(setIsSyncing);

    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  const startSync = async (directoryPath: string, enableWatch = true) => {
    await TauriApi.startSync({ directory_path: directoryPath, enable_watch: enableWatch });
  };

  const stopSync = async () => {
    await TauriApi.stopSync();
  };

  return {
    events,
    isSyncing,
    startSync,
    stopSync,
  };
}
```

## Error Handling Strategy

### Backend Error Conversion

```rust
// backend/src/tauri/error.rs

use crate::domain::base::DomainError;
use crate::application::services::{ImportError, SyncError};
use super::dto::ErrorResponse;

impl From<DomainError> for ErrorResponse {
    fn from(err: DomainError) -> Self {
        let error_type = match &err {
            DomainError::InvalidValue(_) => "ValidationError",
            DomainError::NotFound(_) => "NotFoundError",
            DomainError::BusinessRuleViolation(_) => "BusinessRuleError",
            DomainError::InvalidOperation(_) => "OperationError",
        };

        ErrorResponse::new(err, error_type)
    }
}

impl From<ImportError> for ErrorResponse {
    fn from(err: ImportError) -> Self {
        let error_type = match &err {
            ImportError::InvalidDirectory(_) => "ValidationError",
            ImportError::FileSystem(_) => "FileSystemError",
            ImportError::Parse(_) => "ParseError",
            ImportError::Repository(_) => "RepositoryError",
        };

        ErrorResponse::new(err, error_type)
    }
}

impl From<SyncError> for ErrorResponse {
    fn from(err: SyncError) -> Self {
        let error_type = match &err {
            SyncError::FileSystem(_) => "FileSystemError",
            SyncError::Parse(_) => "ParseError",
            SyncError::Repository(_) => "RepositoryError",
            SyncError::Watcher(_) => "WatcherError",
        };

        ErrorResponse::new(err, error_type)
    }
}
```

### Frontend Error Display

```typescript
// frontend/src/utils/error-messages.ts

export function getErrorMessage(error: Error): string {
  const message = error.message;

  if (message.startsWith('ValidationError:')) {
    return 'Invalid input. Please check your data.';
  } else if (message.startsWith('FileSystemError:')) {
    return 'Unable to access file system. Check permissions.';
  } else if (message.startsWith('RepositoryError:')) {
    return 'Database error. Please try again.';
  } else {
    return 'An unexpected error occurred.';
  }
}
```

## Testing Strategy

### Backend Command Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tauri::test::mock_builder;

    #[tokio::test]
    async fn test_import_directory_command() {
        let app = mock_builder().build().unwrap();

        let result = import_directory(
            app.handle(),
            /* state */,
            ImportRequest {
                directory_path: "./test-fixtures/sample-logseq".to_string(),
            },
        ).await;

        assert!(result.is_ok());
        let summary = result.unwrap();
        assert!(summary.successful > 0);
    }
}
```

### Frontend Integration Tests

```typescript
// frontend/tests/tauri-api.test.ts

import { describe, it, expect, vi } from 'vitest';
import { TauriApi } from '../src/lib/tauri-api';

describe('TauriApi', () => {
  it('should import directory', async () => {
    const summary = await TauriApi.importDirectory({
      directory_path: '/path/to/logseq',
    });

    expect(summary.total_files).toBeGreaterThan(0);
  });

  it('should listen to import progress', async () => {
    const callback = vi.fn();
    await TauriApi.onImportProgress(callback);

    // Trigger import...
    // expect(callback).toHaveBeenCalled();
  });
});
```

## Performance Considerations

### Optimizations

1. **Async commands:** All commands are async to avoid blocking UI
2. **Event streaming:** Use Tauri events instead of polling
3. **Batch operations:** Group database operations in transactions
4. **Connection pooling:** Reuse SQLite connections via Arc<Mutex>
5. **Background tasks:** Run long operations (sync watching) in separate tasks

### Expected Performance

- **Command latency:** <10ms for simple queries, <100ms for complex operations
- **Event latency:** <5ms from Rust → Frontend
- **Import throughput:** ~10-20 files/second (with parsing + DB writes)
- **Sync latency:** <100ms from file change → event emitted

## Rollout Plan

### Phase 1: Foundation ✅
- [ ] Setup Tauri project structure
- [ ] Define DTOs and mappers
- [ ] Implement AppState management
- [ ] Add error handling utilities

### Phase 2: Core Commands ✅
- [ ] Implement import commands
- [ ] Implement sync commands
- [ ] Implement page query commands
- [ ] Add event emitters

### Phase 3: Frontend Integration ✅
- [ ] Create TypeScript types
- [ ] Build TauriApi client
- [ ] Create React hooks (useImport, useSync)
- [ ] Build UI components

### Phase 4: Testing & Polish ✅
- [ ] Backend command tests
- [ ] Frontend integration tests
- [ ] Error handling tests
- [ ] Performance profiling

## Open Questions

1. **Concurrent imports:** Should we allow multiple imports simultaneously?
2. **Event batching:** Should we batch rapid sync events to avoid overwhelming UI?
3. **Command cancellation:** How to cancel long-running operations (import, sync)?
4. **Offline handling:** How to handle when backend is unresponsive?
5. **State persistence:** Should AppState settings be persisted to DB or config file?

## Future Enhancements

- **Command queue:** Queue commands when backend is busy
- **Optimistic updates:** Update UI before backend confirms
- **Command history:** Track all user actions for undo/redo
- **WebSocket alternative:** Use WebSocket for even lower latency
- **Multi-window support:** Sync state across multiple app windows
- **Plugin system:** Allow extending commands via plugins

## References

- Tauri documentation: https://tauri.app/v2/guides/
- Tauri state management: https://tauri.app/v2/guides/features/state/
- Tauri events: https://tauri.app/v2/guides/features/events/
- Tauri commands: https://tauri.app/v2/guides/features/commands/
