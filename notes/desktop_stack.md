# SolidJS + Vite + Tauri Setup

Here's what a SolidJS with Vite setup looks like in Tauri:

## **Project Structure**

```
tauri-app/
├── src/                    # SolidJS frontend code
│   ├── App.tsx
│   ├── index.tsx
│   └── components/
├── src-tauri/             # Rust backend code
│   ├── src/
│   │   └── main.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
├── vite.config.ts
├── package.json
└── index.html
```

## **Initial Setup**

**1. Create a new Vite + SolidJS project:**

```bash
npm create vite@latest tauri-app -- --template solid-ts
cd tauri-app
npm install
```

**2. Add Tauri:**

```bash
npm install -D @tauri-apps/cli
npm install @tauri-apps/api
npx tauri init
```

During initialization, configure:

- **Dev command:** `npm run dev`
- **Build command:** `npm run build`
- **Dev path:** `http://localhost:5173`
- **Dist directory:** `dist`

## **Configuration Files**

**`vite.config.ts`:**

```typescript
import { defineConfig } from "vite";
import solid from "vite-plugin-solid";

export default defineConfig({
  plugins: [solid()],

  // Tauri expects a fixed port for development
  server: {
    port: 5173,
    strictPort: true,
  },

  // Vite options tailored for Tauri
  envPrefix: ["VITE_", "TAURI_"],

  build: {
    // Tauri uses Chromium on Windows and WebKit on macOS and Linux
    target: process.env.TAURI_PLATFORM == "windows" ? "chrome105" : "safari13",
    minify: !process.env.TAURI_DEBUG ? "esbuild" : false,
    sourcemap: !!process.env.TAURI_DEBUG,
  },
});
```

**`src-tauri/tauri.conf.json`:**

```json
{
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devPath": "http://localhost:5173",
    "distDir": "../dist"
  }
}
```

## **Example SolidJS Component with Tauri API**

**`src/App.tsx`:**

```typescript
import { createSignal, For } from "solid-js";
import { invoke } from "@tauri-apps/api/tauri";

interface SearchResult {
  title: string;
  content: string;
  similarity: number;
}

function App() {
  const [query, setQuery] = createSignal("");
  const [results, setResults] = createSignal<SearchResult[]>([]);
  const [loading, setLoading] = createSignal(false);

  const handleSearch = async () => {
    setLoading(true);
    try {
      // Call Rust backend function
      const searchResults = await invoke<SearchResult[]>("search_logseq", {
        query: query(),
      });
      setResults(searchResults);
    } catch (error) {
      console.error("Search failed:", error);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div class="container">
      <h1>Logseq Search</h1>

      <div class="search-box">
        <input
          type="text"
          value={query()}
          onInput={(e) => setQuery(e.currentTarget.value)}
          onKeyPress={(e) => e.key === "Enter" && handleSearch()}
          placeholder="Search your notes..."
        />
        <button onClick={handleSearch} disabled={loading()}>
          {loading() ? "Searching..." : "Search"}
        </button>
      </div>

      <div class="results">
        <For each={results()}>
          {(result) => (
            <div class="result-item">
              <h3>{result.title}</h3>
              <p>{result.content}</p>
              <span class="similarity">
                Similarity: {(result.similarity * 100).toFixed(1)}%
              </span>
            </div>
          )}
        </For>
      </div>
    </div>
  );
}

export default App;
```

**`src-tauri/src/main.rs`:**

```rust
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct SearchResult {
    title: String,
    content: String,
    similarity: f32,
}

#[tauri::command]
fn search_logseq(query: String) -> Result<Vec<SearchResult>, String> {
    // Your search logic here
    // This would include semantic search, URL indexing, etc.

    Ok(vec![
        SearchResult {
            title: "Example Note".to_string(),
            content: "This is an example result".to_string(),
            similarity: 0.95,
        }
    ])
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![search_logseq])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## **Development Workflow**

**Run development server:**

```bash
npm run tauri dev
```

This starts both:

- Vite dev server with hot module replacement
- Tauri window displaying your app

**Build for production:**

```bash
npm run tauri build
```

## **Key Benefits of This Stack**

- **Minimal JS bundle** - SolidJS compiles to highly optimized vanilla JavaScript
- **Fast HMR** - Vite provides instant hot module replacement during development[^1][^2]
- **Type-safe** - TypeScript across frontend and Rust backend
- **Native performance** - Rust backend handles heavy operations (semantic search, indexing)
- **Small app size** - Compiled apps are typically 3-5MB

## **Available Templates**

Several community templates exist to jumpstart your project:[^3][^4]

- `tauri-solid-ts-tailwind-vite` - Includes TailwindCSS, ESLint, Prettier
- `tauri-solidjs-poc` - Simple proof of concept
- `SolidJSTauri` - Fully-featured example with best practices

You can clone these as starting points rather than setting up from scratch.

[^1]: [Create a Project](https://v2.tauri.app/start/create-project/) (29%)
[^2]: [Vite | Tauri](https://v2.tauri.app/start/frontend/vite/) (28%)
[^3]: [Small Tauri SolidJS Example feat. Vite - GitHub](https://github.com/lukethacoder/tauri-solid-example) (24%)
[^4]: [GitHub - ZanzyTHEbar/SolidJSTauri: This is a fully-featured Tauri ...](https://github.com/ZanzyTHEbar/SolidJSTauri) (19%)

# Frontend-Backend Communication in Tauri

Tauri uses **Asynchronous Message Passing** for Inter-Process Communication (IPC) between the JavaScript frontend and Rust backend.[^1] There are two primary communication methods:

## **1. Commands (Frontend → Backend)**

Commands are the primary way to call Rust functions from your frontend. They're **type-safe** and support arguments and return values.[^2]

### **Rust Side:**

```rust
// src-tauri/src/main.rs

#[tauri::command]
fn search_notes(query: String, limit: usize) -> Result<Vec<String>, String> {
    // Your search logic here
    if query.is_empty() {
        return Err("Query cannot be empty".to_string());
    }

    // Perform search
    let results = vec!["Result 1".to_string(), "Result 2".to_string()];
    Ok(results)
}

#[tauri::command]
async fn async_search(query: String) -> Result<String, String> {
    // Async operations work too
    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    Ok(format!("Searched for: {}", query))
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            search_notes,
            async_search
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### **Frontend Side:**

```typescript
import { invoke } from "@tauri-apps/api/tauri";

// Simple invocation
const results = await invoke<string[]>("search_notes", {
  query: "my search",
  limit: 10,
});

// With error handling
try {
  const result = await invoke<string>("async_search", {
    query: "test",
  });
  console.log(result);
} catch (error) {
  console.error("Command failed:", error);
}
```

**Key Points:**

- Commands are **async by default**[^5]
- Arguments must be passed as an object with keys matching Rust parameter names
- Return types are serialized/deserialized automatically using JSON
- Type-safe with proper TypeScript types

## **2. Events (Bidirectional Communication)**

The event system allows **message passing in both directions** and supports multiple listeners.[^6][^3]

### **Backend → Frontend (Emitting from Rust):**

```rust
use tauri::Manager;

#[tauri::command]
fn start_indexing(app: tauri::AppHandle) {
    std::thread::spawn(move || {
        for i in 0..100 {
            // Emit progress updates
            app.emit_all("indexing-progress", i).unwrap();
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        app.emit_all("indexing-complete", "Done!").unwrap();
    });
}

// Or emit to a specific window
#[tauri::command]
fn notify_window(window: tauri::Window) {
    window.emit("notification", "Hello from Rust!").unwrap();
}
```

### **Frontend Listening:**

```typescript
import { listen } from "@tauri-apps/api/event";

// Listen for events
const unlisten = await listen<number>("indexing-progress", (event) => {
  console.log("Progress:", event.payload);
  updateProgressBar(event.payload);
});

await listen<string>("indexing-complete", (event) => {
  console.log("Indexing done:", event.payload);
});

// Clean up listener when done
unlisten();
```

### **Frontend → Backend (Emitting from Frontend):**

```typescript
import { emit } from "@tauri-apps/api/event";

// Emit event to backend
await emit("user-action", { action: "search", query: "test" });
```

```rust
use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let app_handle = app.handle();

            // Listen for frontend events
            app_handle.listen_global("user-action", |event| {
                println!("Received event: {:?}", event.payload());
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

## **3. Channels (Streaming Data)**

For **continuous data streams**, Tauri v2 supports channels:[^4]

```rust
#[tauri::command]
async fn stream_search_results(
    query: String,
    channel: tauri::ipc::Channel
) -> Result<(), String> {
    for i in 0..10 {
        channel.send(format!("Result {}", i)).unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    Ok(())
}
```

```typescript
import { invoke, Channel } from "@tauri-apps/api/tauri";

const channel = new Channel<string>();
channel.onmessage = (message) => {
  console.log("Received:", message);
};

await invoke("stream_search_results", {
  query: "test",
  channel,
});
```

## **When to Use Each Method**

| Method       | Use Case                                                       | Direction          |
| ------------ | -------------------------------------------------------------- | ------------------ |
| **Commands** | Request-response operations, fetching data, performing actions | Frontend → Backend |
| **Events**   | Progress updates, notifications, pub-sub patterns              | Bidirectional      |
| **Channels** | Streaming data, real-time updates, large datasets              | Backend → Frontend |

## **For Your Search App**

I'd recommend:

- **Commands** for search queries and indexing operations
- **Events** for indexing progress updates and notifications
- **Channels** if you need to stream search results incrementally

This gives you type-safe, performant communication with minimal overhead.[^1][^2]

[^1]: [Inter-Process Communication](https://v2.tauri.app/concept/inter-process-communication/) (32%)
[^2]: [Calling Rust from the Frontend - Tauri](https://v2.tauri.app/develop/calling-rust/) (32%)
[^3]: [Events | Tauri v1](https://v1.tauri.app/v1/guides/features/events) (13%)
[^4]: [Calling the Frontend from Rust](https://v2.tauri.app/develop/calling-frontend/) (11%)
[^5]: [tauri | Tauri v1](https://tauri.app/v1/api/js/tauri/) (7%)
[^6]: [Events | Tauri v1](https://tauri.app/v1/guides/features/events/) (5%)

# Automated Testing Best Practices for Tauri Apps

Yes, it absolutely makes sense to have automated tests for your desktop app, even with good backend coverage. Here's a comprehensive testing strategy:

## **Testing Layers**

### **1. Unit Tests (Rust Backend)**

You already have this covered, but ensure you're testing:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_logic() {
        let result = search_notes("query", 10);
        assert!(result.is_ok());
    }
}
```

### **2. Integration Tests (Frontend-Backend Communication)**

Tauri provides a **mock runtime** for testing IPC communication without launching the actual webview.[^1]

**Rust side:**

```rust
#[cfg(test)]
mod tests {
    use tauri::test::{mock_builder, mock_context, noop_assets};

    #[test]
    fn test_command() {
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![your_command])
            .build(mock_context(noop_assets()))
            .unwrap();

        // Test your commands
    }
}
```

**Frontend side with mocking:**

```typescript
import { mockIPC } from "@tauri-apps/api/mocks";
import { invoke } from "@tauri-apps/api/tauri";

describe("Search functionality", () => {
  beforeEach(() => {
    mockIPC((cmd, args) => {
      if (cmd === "search_notes") {
        return ["result1", "result2"];
      }
    });
  });

  afterEach(() => {
    // Clear mocks after each test
    clearMocks();
  });

  it("should return search results", async () => {
    const results = await invoke("search_notes", { query: "test" });
    expect(results).toHaveLength(2);
  });
});
```

### **3. End-to-End Tests (Full Desktop App)**

E2E tests validate the complete user experience using **WebDriver**.[^3][^2]

**Setup with WebDriverIO:**

```bash
npm install -D @wdio/cli
npx wdio config
npm install -D tauri-driver
```

**`wdio.conf.js`:**

```javascript
export const config = {
  specs: ["./e2e-tests/**/*.spec.js"],
  capabilities: [
    {
      maxInstances: 1,
      "tauri:options": {
        application: "./src-tauri/target/release/your-app",
      },
    },
  ],
  port: 4445,
  services: [
    [
      "tauri",
      {
        tauriDriver: {
          logLevel: "info",
        },
      },
    ],
  ],
  framework: "mocha",
  reporters: ["spec"],
};
```

**Example E2E test:**

```javascript
describe("Logseq Search App", () => {
  it("should perform a search", async () => {
    const searchInput = await $('input[type="text"]');
    await searchInput.setValue("my query");

    const searchButton = await $("button");
    await searchButton.click();

    // Wait for results
    await browser.waitUntil(async () => (await $$(".result-item")).length > 0, {
      timeout: 5000,
    });

    const results = await $$(".result-item");
    expect(results.length).toBeGreaterThan(0);
  });

  it("should handle empty search gracefully", async () => {
    const searchButton = await $("button");
    await searchButton.click();

    const errorMessage = await $(".error-message");
    expect(await errorMessage.isDisplayed()).toBe(true);
  });
});
```

## **CI/CD Integration**

**GitHub Actions example:**[^6]

```yaml
name: Test Tauri App

on: [push, pull_request]

jobs:
  test:
    strategy:
      matrix:
        platform: [ubuntu-latest, windows-latest]

    runs-on: ${{ matrix.platform }}

    steps:
      - uses: actions/checkout@v4

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: 20

      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install dependencies (Ubuntu)
        if: matrix.platform == 'ubuntu-latest'
        run: |
          sudo apt-get update
          sudo apt-get install -y webkit2gtk-4.1 libayatana-appindicator3-dev

      - name: Install frontend dependencies
        run: npm install

      - name: Run Rust tests
        run: cd src-tauri && cargo test

      - name: Run frontend tests
        run: npm test

      - name: Build app
        run: npm run tauri build

      - name: Run E2E tests
        run: cd e2e-tests && npm test
```

## **Best Practices**

### **What to Test:**

✅ **Critical user flows** - Search, indexing, URL handling  
✅ **Error handling** - Empty queries, failed searches, network errors  
✅ **Performance** - Search response times, UI responsiveness  
✅ **Cross-platform behavior** - Test on Windows, Linux, macOS  
✅ **IPC communication** - Command invocations, event handling

### **What NOT to Test:**

❌ **Framework internals** - Don't test SolidJS/Tauri itself  
❌ **Visual styling** - Unless critical to functionality  
❌ **Every edge case** - Focus on high-value scenarios

## **Recommended Testing Strategy for Your App**

Given your search app requirements:

1. **Unit tests (80% coverage)** - Rust backend logic

   - Search algorithms
   - URL parsing and indexing
   - Markdown parsing

2. **Integration tests (key flows)** - Mock IPC[^4]

   - Search command with various inputs
   - Indexing operations
   - Error scenarios

3. **E2E tests (critical paths)** - WebDriver[^3][^5]
   - Basic search flow
   - Semantic similarity search
   - URL search functionality
   - Performance under load

## **Is It Worth It?**

**Yes, for these reasons:**

- **Catches integration bugs** - Backend tests won't catch IPC issues or UI bugs
- **Regression prevention** - Ensures updates don't break existing functionality
- **Cross-platform confidence** - Validates behavior across Windows/Linux/macOS
- **Refactoring safety** - Allows confident code changes
- **Documentation** - Tests serve as usage examples

**However:**

- E2E tests are **slower and more brittle** than unit tests
- Focus on **high-value user flows** rather than comprehensive coverage
- Maintain a **testing pyramid**: Many unit tests, some integration tests, few E2E tests

For your search app, I'd recommend **70% unit, 20% integration, 10% E2E** as a good balance.[^3][^1]

[^1]: [Tests - Tauri](https://v2.tauri.app/develop/tests/) (42%)
[^2]: [Techniques for testing inside and outside the Tauri runtime](https://v2.tauri.app/ja/develop/tests/) (16%)
[^3]: [Testing - The Tauri Documentation WIP - GitHub Pages](https://jonaskruckenberg.github.io/tauri-docs-wip/development/testing.html) (12%)
[^4]: [Mock Tauri APIs](https://v2.tauri.app/develop/tests/mocking/) (12%)
[^5]: [WebDriver - Tauri](https://v2.tauri.app/develop/tests/webdriver/) (11%)
[^6]: [Continuous Integration - Tauri](https://v2.tauri.app/develop/tests/webdriver/ci/) (7%)
