/// Domain events
use super::base::DomainEvent;
use super::value_objects::{BlockId, PageId};

/// Event emitted when a new page is created
#[derive(Debug, Clone)]
pub struct PageCreated {
    pub page_id: PageId,
    pub title: String,
}

impl DomainEvent for PageCreated {
    fn event_type(&self) -> &'static str {
        "PageCreated"
    }

    fn aggregate_id(&self) -> String {
        self.page_id.as_str().to_string()
    }
}

/// Event emitted when a page is updated
#[derive(Debug, Clone)]
pub struct PageUpdated {
    pub page_id: PageId,
    pub title: Option<String>,
}

impl DomainEvent for PageUpdated {
    fn event_type(&self) -> &'static str {
        "PageUpdated"
    }

    fn aggregate_id(&self) -> String {
        self.page_id.as_str().to_string()
    }
}

/// Event emitted when a page is deleted
#[derive(Debug, Clone)]
pub struct PageDeleted {
    pub page_id: PageId,
}

impl DomainEvent for PageDeleted {
    fn event_type(&self) -> &'static str {
        "PageDeleted"
    }

    fn aggregate_id(&self) -> String {
        self.page_id.as_str().to_string()
    }
}

/// Event emitted when a block is added to a page
#[derive(Debug, Clone)]
pub struct BlockAdded {
    pub page_id: PageId,
    pub block_id: BlockId,
    pub parent_block_id: Option<BlockId>,
}

impl DomainEvent for BlockAdded {
    fn event_type(&self) -> &'static str {
        "BlockAdded"
    }

    fn aggregate_id(&self) -> String {
        self.page_id.as_str().to_string()
    }
}

/// Event emitted when a block is updated
#[derive(Debug, Clone)]
pub struct BlockUpdated {
    pub page_id: PageId,
    pub block_id: BlockId,
}

impl DomainEvent for BlockUpdated {
    fn event_type(&self) -> &'static str {
        "BlockUpdated"
    }

    fn aggregate_id(&self) -> String {
        self.page_id.as_str().to_string()
    }
}

/// Event emitted when a block is removed from a page
#[derive(Debug, Clone)]
pub struct BlockRemoved {
    pub page_id: PageId,
    pub block_id: BlockId,
}

impl DomainEvent for BlockRemoved {
    fn event_type(&self) -> &'static str {
        "BlockRemoved"
    }

    fn aggregate_id(&self) -> String {
        self.page_id.as_str().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_created_event() {
        let page_id = PageId::new("page-1").unwrap();
        let event = PageCreated {
            page_id: page_id.clone(),
            title: "Test Page".to_string(),
        };

        assert_eq!(event.event_type(), "PageCreated");
        assert_eq!(event.aggregate_id(), "page-1");
    }

    #[test]
    fn test_page_updated_event() {
        let page_id = PageId::new("page-1").unwrap();
        let event = PageUpdated {
            page_id: page_id.clone(),
            title: Some("Updated Title".to_string()),
        };

        assert_eq!(event.event_type(), "PageUpdated");
        assert_eq!(event.aggregate_id(), "page-1");
    }

    #[test]
    fn test_page_deleted_event() {
        let page_id = PageId::new("page-1").unwrap();
        let event = PageDeleted {
            page_id: page_id.clone(),
        };

        assert_eq!(event.event_type(), "PageDeleted");
        assert_eq!(event.aggregate_id(), "page-1");
    }

    #[test]
    fn test_block_added_event() {
        let page_id = PageId::new("page-1").unwrap();
        let block_id = BlockId::new("block-1").unwrap();
        let event = BlockAdded {
            page_id: page_id.clone(),
            block_id: block_id.clone(),
            parent_block_id: None,
        };

        assert_eq!(event.event_type(), "BlockAdded");
        assert_eq!(event.aggregate_id(), "page-1");
    }

    #[test]
    fn test_block_updated_event() {
        let page_id = PageId::new("page-1").unwrap();
        let block_id = BlockId::new("block-1").unwrap();
        let event = BlockUpdated {
            page_id: page_id.clone(),
            block_id: block_id.clone(),
        };

        assert_eq!(event.event_type(), "BlockUpdated");
        assert_eq!(event.aggregate_id(), "page-1");
    }

    #[test]
    fn test_block_removed_event() {
        let page_id = PageId::new("page-1").unwrap();
        let block_id = BlockId::new("block-1").unwrap();
        let event = BlockRemoved {
            page_id: page_id.clone(),
            block_id: block_id.clone(),
        };

        assert_eq!(event.event_type(), "BlockRemoved");
        assert_eq!(event.aggregate_id(), "page-1");
    }
}
