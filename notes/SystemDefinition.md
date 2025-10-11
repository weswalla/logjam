## High Level Overview

Logjam is a companion app to Logseq. The main thing it provides is an enhanced search experience, not only for notes but for any urls within the logseq note base. It offers both semantic search as well as traditional search, which can be toggled and eventually combined in a hybrid.

## Main Layout and Experience

Like logseq, logjam will have a main panel and a right sidebar panel. The main panel will be the main search interface which will look and feel a lot like a Search Engine (inspired by Kagi vibes). The right panel, which can be toggled open or closed, will be a stackable list of views, just like logseq, where each view could be something different (view of a page / result, more search results, etc. etc.).

## Basic functionality

Logseq notes are made up of markdown files in a file directory. All relevant files are in 2 subdirectories `journals` and `pages`. Logjam will simply read and index these files as well as url within then. It will continuously listen for file change events and handle these events to update the search indexes. Additionally, logjam will support "importing" directories to process and index the entire directory when setting up. Logjam will also be able to check for updates to files in the logseq directory since last opened and index those files and urls.

## Software Architecture and Stack

We are going to use a layered architecture with Domain-Driven Design patterns and principles. This means most things are broken up into domain, application, and infrastructure layers as well as subdomains.

I want to make sure we use best practices for DDD in rust and that we have minimal yet useful set of base DDD abstractions like Result (though rust may already handle this well), Controller, UseCase, Aggregate, Entity, ValueObject, Domain Event, etc. things like this.

### Domain Layer

In logseq, notes are broken up into "Pages" and "Blocks". Pages are markdown files and "Blocks" are bullet points on that page. Every line of text on a "Page" is a "Block". The structure of Blocks on a Page make up a tree, because blocks can be nested/indented. So Blocks have a hierarchy on a page, with parents and children.

Logseq also has page references and tags, which are basically the same thing (a page reference is surrounded by `[[]]` and a tag starts with `#`) but they both just reference a page by title. These references would be within a block.

Url would also be within a block.

The domain layer should be able to capture this rich model of pages, blocks, references and URLs, and understand the hierarchical relationship between them on pages (i.e. for a give URL on a page, know which page references are in parent/ancestor blocks and child/descendent blocks). Vise versa, for a given page reference on a page, know which urls are in parent and child blocks.

Domain layer also defines events of interest, which are things like Page updated etc. etc.

Not sure if Files and File Paths should be part of the domain model or not, but adding here as consideration.

### Application Layer

The application Layer is where all the use-cases are implemented, and know nothing of the infrastructure layer, it only knows about the domain layer.

Since the primary use case as of know is search, we should be able to do the following when performing a search:

- provide a search query
- select whether we want traditional search or semantic search
- select whether we want only pages / blocks in the results or only URLs or both

The other thing we will want to be able to do is for a given url, search for semantically similar ones (the hacky way is to just take the description or whatever other text from the url and perform a link only semantic search for it).

We will also have other things like ImportLogseqDirectory which will initiate and coordinate the import process, which ideally uses a task queue and can provide status updates. As well as SyncLogseqDirectory which will make sure the index is up to date on any changes to it since last using logjam.

### Infrastructure layer

This is where we handle things like reading and writing from files, calling APIs or pulling webpages, persisting data in a db and running queries.

This is also where we will have endpoints, routes, controllers to handle http request from the client.

I plan on using rust for all of the backend (which is where most of the DDD and layered arch will be implemented).

I want to use a fast and performant DB (postgres?) that will be great for these kinds of search engine like queries as well as semantic search (not sure if there should be a separate vector db for that or if things like postgres support vector search as well).
