# Domain Layer

This directory contains the domain layer implementation for Logjam, following Domain-Driven Design (DDD) principles.

## Overview

The domain layer models the core concepts of Logseq notes:
- **Pages**: Markdown files that contain blocks
- **Blocks**: Individual bullet points that form a tree hierarchy
- **Page References**: Links to other pages using `[[page-name]]` or `#tag` syntax
- **URLs**: Web links embedded within blocks

## Structure

### Base (`base.rs`)
Core DDD abstractions:
- `ValueObject`: Trait for immutable value objects
- `Entity`: Trait for objects with identity
- `AggregateRoot`: Trait for aggregate roots that manage consistency boundaries
- `DomainEvent`: Trait for domain events
- `DomainError`: Domain-specific error types
- `DomainResult<T>`: Result type for domain operations

### Value Objects (`value_objects.rs`)
Immutable objects defined by their attributes:
- `PageId`: Unique identifier for a page
- `BlockId`: Unique identifier for a block
- `Url`: A web URL with validation
- `PageReference`: A reference to another page (either `[[page]]` or `#tag`)
- `BlockContent`: The text content of a block
- `IndentLevel`: The indentation level of a block in the hierarchy

### Entities (`entities.rs`)
Objects with identity:
- `Block`: Represents a single bullet point with:
  - Content
  - Hierarchy relationships (parent/children)
  - URLs and page references
  - Indent level

### Aggregates (`aggregates.rs`)
Aggregate roots that ensure consistency:
- `Page`: The main aggregate root that:
  - Contains a tree of blocks
  - Manages block hierarchy
  - Provides methods to traverse relationships
  - Ensures consistency of the block tree

#### Key Page Methods

**Basic Operations:**
- `new(id, title)`: Create a new page
- `add_block(block)`: Add a block to the page
- `get_block(id)`: Retrieve a block by ID
- `remove_block(id)`: Remove a block and its descendants

**Queries:**
- `root_blocks()`: Get all top-level blocks
- `all_blocks()`: Get all blocks in the page
- `all_urls()`: Get all URLs in the page
- `all_page_references()`: Get all page references in the page

**Hierarchy Navigation:**
- `get_ancestors(block_id)`: Get all ancestor blocks (parent to root)
- `get_descendants(block_id)`: Get all descendant blocks (recursive)
- `get_hierarchy_path(block_id)`: Get full path from root to block

**Contextual Queries:**
- `get_urls_with_context()`: Get URLs with their ancestor and descendant page references
- `get_page_references_with_context()`: Get page references with their ancestor and descendant URLs

### Events (`events.rs`)
Domain events that represent things that have happened:
- `PageCreated`: A new page was created
- `PageUpdated`: A page was modified
- `PageDeleted`: A page was deleted
- `BlockAdded`: A block was added to a page
- `BlockUpdated`: A block was modified
- `BlockRemoved`: A block was removed from a page

## Example Usage

```rust
use backend::domain::*;

// Create a page
let page_id = PageId::new("my-page").unwrap();
let mut page = Page::new(page_id, "My Page".to_string());

// Create a root block with a page reference
let block1_id = BlockId::new("block-1").unwrap();
let mut block1 = Block::new_root(
    block1_id.clone(),
    BlockContent::new("Check out [[another-page]]"),
);
block1.add_page_reference(PageReference::from_brackets("another-page").unwrap());
page.add_block(block1).unwrap();

// Create a child block with a URL
let block2_id = BlockId::new("block-2").unwrap();
let mut block2 = Block::new_child(
    block2_id.clone(),
    BlockContent::new("Visit https://example.com"),
    block1_id.clone(),
    IndentLevel::new(1),
);
block2.add_url(Url::new("https://example.com").unwrap());
page.add_block(block2).unwrap();

// Query the page
let all_urls = page.all_urls();
let all_refs = page.all_page_references();

// Get contextual information
let urls_with_context = page.get_urls_with_context();
for (url, ancestor_refs, descendant_refs) in urls_with_context {
    println!("URL: {}", url);
    println!("  Ancestor page refs: {:?}", ancestor_refs);
    println!("  Descendant page refs: {:?}", descendant_refs);
}
```

## Testing

All modules include comprehensive unit tests. Run them with:

```bash
cargo test
```

See `tests/integration_test.rs` for a complete example that recreates the Logseq hierarchy from the issue description.

## Design Principles

1. **Immutability**: Value objects are immutable
2. **Encapsulation**: The Page aggregate ensures consistency of the block tree
3. **Rich Domain Model**: Blocks and Pages understand their relationships
4. **Type Safety**: Rust's type system prevents invalid states
5. **Testability**: All domain logic is pure and easily testable

## Future Enhancements

- File path management (may be part of domain or infrastructure)
- More sophisticated URL parsing and validation
- Block ordering within parent blocks
- Timestamp tracking for blocks and pages
- More domain events and event sourcing support
