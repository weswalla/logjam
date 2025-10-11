/// Integration tests for the domain layer
/// This test recreates the example from the issue description

#[cfg(test)]
mod tests {
    use backend::domain::*;

    /// Test the example from the issue:
    /// ```
    /// - im starting to make some [[notes]] about various things like [[logseq]]
    ///     - https://logseq.com/
    ///     - I'd like to stay up to date on the github repo: https://github.com/logseq/logseq
    ///         - [[worflow]] needs an update thought
    /// - alright again with the updates!
    ///     - [[notes]]
    ///         - https://google.com
    ///             - this is [[evil tech]]
    ///         - more notes
    ///         - https://obsidian.md/
    /// - more bullets!
    ///     - bullet
    /// - [[UI Tools]]
    ///     - https://ui.shadcn.com/
    /// ```
    #[test]
    fn test_logseq_page_hierarchy_example() {
        let page_id = PageId::new("test-page").unwrap();
        let mut page = Page::new(page_id, "Test Page".to_string());

        // Block 1: im starting to make some [[notes]] about various things like [[logseq]]
        let block1_id = BlockId::new("block-1").unwrap();
        let mut block1 = Block::new_root(
            block1_id.clone(),
            BlockContent::new("im starting to make some notes about various things like logseq"),
        );
        block1.add_page_reference(PageReference::from_brackets("notes").unwrap());
        block1.add_page_reference(PageReference::from_brackets("logseq").unwrap());
        page.add_block(block1).unwrap();

        // Block 1.1: https://logseq.com/
        let block1_1_id = BlockId::new("block-1-1").unwrap();
        let mut block1_1 = Block::new_child(
            block1_1_id.clone(),
            BlockContent::new("https://logseq.com/"),
            block1_id.clone(),
            IndentLevel::new(1),
        );
        block1_1.add_url(Url::new("https://logseq.com/").unwrap());
        page.add_block(block1_1).unwrap();

        // Block 1.2: I'd like to stay up to date on the github repo: https://github.com/logseq/logseq
        let block1_2_id = BlockId::new("block-1-2").unwrap();
        let mut block1_2 = Block::new_child(
            block1_2_id.clone(),
            BlockContent::new("I'd like to stay up to date on the github repo"),
            block1_id.clone(),
            IndentLevel::new(1),
        );
        block1_2.add_url(Url::new("https://github.com/logseq/logseq").unwrap());
        page.add_block(block1_2).unwrap();

        // Block 1.2.1: [[worflow]] needs an update thought
        let block1_2_1_id = BlockId::new("block-1-2-1").unwrap();
        let mut block1_2_1 = Block::new_child(
            block1_2_1_id.clone(),
            BlockContent::new("workflow needs an update thought"),
            block1_2_id.clone(),
            IndentLevel::new(2),
        );
        block1_2_1.add_page_reference(PageReference::from_brackets("workflow").unwrap());
        page.add_block(block1_2_1).unwrap();

        // Block 2: alright again with the updates!
        let block2_id = BlockId::new("block-2").unwrap();
        let block2 = Block::new_root(
            block2_id.clone(),
            BlockContent::new("alright again with the updates!"),
        );
        page.add_block(block2).unwrap();

        // Block 2.1: [[notes]]
        let block2_1_id = BlockId::new("block-2-1").unwrap();
        let mut block2_1 = Block::new_child(
            block2_1_id.clone(),
            BlockContent::new("notes"),
            block2_id.clone(),
            IndentLevel::new(1),
        );
        block2_1.add_page_reference(PageReference::from_brackets("notes").unwrap());
        page.add_block(block2_1).unwrap();

        // Block 2.1.1: https://google.com
        let block2_1_1_id = BlockId::new("block-2-1-1").unwrap();
        let mut block2_1_1 = Block::new_child(
            block2_1_1_id.clone(),
            BlockContent::new("https://google.com"),
            block2_1_id.clone(),
            IndentLevel::new(2),
        );
        block2_1_1.add_url(Url::new("https://google.com").unwrap());
        page.add_block(block2_1_1).unwrap();

        // Block 2.1.1.1: this is [[evil tech]]
        let block2_1_1_1_id = BlockId::new("block-2-1-1-1").unwrap();
        let mut block2_1_1_1 = Block::new_child(
            block2_1_1_1_id.clone(),
            BlockContent::new("this is evil tech"),
            block2_1_1_id.clone(),
            IndentLevel::new(3),
        );
        block2_1_1_1.add_page_reference(PageReference::from_brackets("evil tech").unwrap());
        page.add_block(block2_1_1_1).unwrap();

        // Block 2.1.2: more notes
        let block2_1_2_id = BlockId::new("block-2-1-2").unwrap();
        let block2_1_2 = Block::new_child(
            block2_1_2_id.clone(),
            BlockContent::new("more notes"),
            block2_1_id.clone(),
            IndentLevel::new(2),
        );
        page.add_block(block2_1_2).unwrap();

        // Block 2.1.3: https://obsidian.md/
        let block2_1_3_id = BlockId::new("block-2-1-3").unwrap();
        let mut block2_1_3 = Block::new_child(
            block2_1_3_id.clone(),
            BlockContent::new("https://obsidian.md/"),
            block2_1_id.clone(),
            IndentLevel::new(2),
        );
        block2_1_3.add_url(Url::new("https://obsidian.md/").unwrap());
        page.add_block(block2_1_3).unwrap();

        // Block 3: more bullets!
        let block3_id = BlockId::new("block-3").unwrap();
        let block3 = Block::new_root(
            block3_id.clone(),
            BlockContent::new("more bullets!"),
        );
        page.add_block(block3).unwrap();

        // Block 3.1: bullet
        let block3_1_id = BlockId::new("block-3-1").unwrap();
        let block3_1 = Block::new_child(
            block3_1_id.clone(),
            BlockContent::new("bullet"),
            block3_id.clone(),
            IndentLevel::new(1),
        );
        page.add_block(block3_1).unwrap();

        // Block 4: [[UI Tools]]
        let block4_id = BlockId::new("block-4").unwrap();
        let mut block4 = Block::new_root(
            block4_id.clone(),
            BlockContent::new("UI Tools"),
        );
        block4.add_page_reference(PageReference::from_brackets("UI Tools").unwrap());
        page.add_block(block4).unwrap();

        // Block 4.1: https://ui.shadcn.com/
        let block4_1_id = BlockId::new("block-4-1").unwrap();
        let mut block4_1 = Block::new_child(
            block4_1_id.clone(),
            BlockContent::new("https://ui.shadcn.com/"),
            block4_id.clone(),
            IndentLevel::new(1),
        );
        block4_1.add_url(Url::new("https://ui.shadcn.com/").unwrap());
        page.add_block(block4_1).unwrap();

        // Now test the queries

        // 1. Get all URLs in the page
        let all_urls = page.all_urls();
        assert_eq!(all_urls.len(), 5);

        // 2. Get all page references in the page
        let all_refs = page.all_page_references();
        assert_eq!(all_refs.len(), 6); // notes, logseq, workflow, notes (again), evil tech, UI Tools

        // 3. Test getting URLs with their context (ancestor and descendant page refs)
        let urls_with_context = page.get_urls_with_context();

        // Find https://google.com and check its context
        let google_url_context = urls_with_context
            .iter()
            .find(|(url, _, _)| url.as_str() == "https://google.com")
            .unwrap();

        let (_, ancestor_refs, descendant_refs) = google_url_context;

        // Ancestor refs should include [[notes]] from block 2.1
        assert_eq!(ancestor_refs.len(), 1);
        assert_eq!(ancestor_refs[0].title(), "notes");

        // Descendant refs should include [[evil tech]] from block 2.1.1.1
        assert_eq!(descendant_refs.len(), 1);
        assert_eq!(descendant_refs[0].title(), "evil tech");

        // 4. Test getting page references with their context (ancestor and descendant URLs)
        let refs_with_context = page.get_page_references_with_context();

        // Find [[workflow]] reference and check its context
        let workflow_ref_context = refs_with_context
            .iter()
            .find(|(ref_val, _, _)| ref_val.title() == "workflow")
            .unwrap();

        let (_, ancestor_urls, descendant_urls) = workflow_ref_context;

        // Ancestor URLs should include the github URL from block 1.2
        assert_eq!(ancestor_urls.len(), 1);
        assert_eq!(ancestor_urls[0].as_str(), "https://github.com/logseq/logseq");

        // No descendant URLs for workflow
        assert_eq!(descendant_urls.len(), 0);

        // 5. Test hierarchy path
        let path = page.get_hierarchy_path(&block2_1_1_1_id);
        assert_eq!(path.len(), 4); // block-2 -> block-2-1 -> block-2-1-1 -> block-2-1-1-1

        // 6. Test getting all descendants
        let descendants = page.get_descendants(&block2_1_id);
        assert_eq!(descendants.len(), 4); // block-2-1-1, block-2-1-1-1, block-2-1-2, block-2-1-3
    }

    #[test]
    fn test_page_reference_filtering() {
        let page_id = PageId::new("test-page").unwrap();
        let mut page = Page::new(page_id, "Test Page".to_string());

        let block_id = BlockId::new("block-1").unwrap();
        let mut block = Block::new_root(
            block_id.clone(),
            BlockContent::new("Test block"),
        );

        // Add both page references and tags
        block.add_page_reference(PageReference::from_brackets("regular-page").unwrap());
        block.add_page_reference(PageReference::from_tag("tag-page").unwrap());

        page.add_block(block).unwrap();

        let refs = page.all_page_references();
        assert_eq!(refs.len(), 2);

        // Verify we can distinguish between tags and page references
        let tag_count = refs.iter().filter(|r| r.is_tag()).count();
        let page_ref_count = refs.iter().filter(|r| r.is_page_reference()).count();

        assert_eq!(tag_count, 1);
        assert_eq!(page_ref_count, 1);
    }
}
