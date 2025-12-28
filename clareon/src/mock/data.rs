// SPDX-FileContributor: Daniel Vrátil <me@dvratil.cz>
//
// SPDX-License-Identifier: GPL-3.0-or-later

use chrono::Utc;

#[derive(Debug, Clone)]
pub struct MockConversation {
    pub id: i64,
    pub title: String,
    pub updated_at: i64,
    pub model: String,
    pub message_count: i64,
}

#[derive(Debug, Clone)]
pub struct MockMessage {
    pub id: i64,
    pub role: String, // "user" or "assistant"
    pub text_content: String,
    pub created_at: i64,
}

/// Generate mock conversation data
pub fn mock_conversations() -> Vec<MockConversation> {
    let now = Utc::now().timestamp();

    vec![
        MockConversation {
            id: 1,
            title: "Rust async patterns".to_string(),
            updated_at: now - 3600, // 1 hour ago
            model: "claude-sonnet-4".to_string(),
            message_count: 12,
        },
        MockConversation {
            id: 2,
            title: "QML layout design".to_string(),
            updated_at: now - 7200, // 2 hours ago
            model: "claude-sonnet-4".to_string(),
            message_count: 8,
        },
        MockConversation {
            id: 3,
            title: "Debugging SQLite queries".to_string(),
            updated_at: now - 86400, // 1 day ago
            model: "claude-sonnet-4".to_string(),
            message_count: 15,
        },
        MockConversation {
            id: 4,
            title: "Kirigami components overview".to_string(),
            updated_at: now - 172800, // 2 days ago
            model: "claude-sonnet-4".to_string(),
            message_count: 6,
        },
        MockConversation {
            id: 5,
            title: "Git workflow best practices".to_string(),
            updated_at: now - 259200, // 3 days ago
            model: "claude-haiku".to_string(),
            message_count: 10,
        },
        MockConversation {
            id: 6,
            title: "Linux desktop integration".to_string(),
            updated_at: now - 432000, // 5 days ago
            model: "claude-sonnet-4".to_string(),
            message_count: 20,
        },
        MockConversation {
            id: 7,
            title: "Error handling in Rust".to_string(),
            updated_at: now - 604800, // 1 week ago
            model: "claude-sonnet-4".to_string(),
            message_count: 14,
        },
        MockConversation {
            id: 8,
            title: "CSS grid vs flexbox".to_string(),
            updated_at: now - 1209600, // 2 weeks ago
            model: "claude-haiku".to_string(),
            message_count: 7,
        },
    ]
}

/// Generate mock messages for a conversation
pub fn mock_messages() -> Vec<MockMessage> {
    let now = Utc::now().timestamp();

    vec![
        MockMessage {
            id: 1,
            role: "user".to_string(),
            text_content: "Hi! Can you explain how async/await works in Rust?".to_string(),
            created_at: now - 3600,
        },
        MockMessage {
            id: 2,
            role: "assistant".to_string(),
            text_content: "Absolutely! Async/await in Rust is a way to write asynchronous code that looks and behaves like synchronous code. Here are the key concepts:\n\n1. **Futures**: A Future represents a value that will be available at some point in the future. It's Rust's abstraction for asynchronous computation.\n\n2. **async fn**: When you mark a function as `async`, it returns a Future instead of executing immediately.\n\n3. **await**: The `.await` keyword is used to suspend execution until a Future completes.\n\nWould you like me to show you a practical example?".to_string(),
            created_at: now - 3590,
        },
        MockMessage {
            id: 3,
            role: "user".to_string(),
            text_content: "Yes, please! A practical example would be great.".to_string(),
            created_at: now - 3550,
        },
        MockMessage {
            id: 4,
            role: "assistant".to_string(),
            text_content: "Here's a practical example using tokio:\n\n```rust\nuse tokio::time::{sleep, Duration};\n\n#[tokio::main]\nasync fn main() {\n    println!(\"Starting...\");\n    \n    let result = fetch_data().await;\n    println!(\"Got result: {}\", result);\n}\n\nasync fn fetch_data() -> String {\n    // Simulate an async operation (like a network request)\n    sleep(Duration::from_secs(2)).await;\n    \"Hello from async!\".to_string()\n}\n```\n\nIn this example:\n- `fetch_data` is an async function that simulates a 2-second delay\n- The `.await` keyword pauses execution until the sleep completes\n- The main function is also async and awaits the result".to_string(),
            created_at: now - 3500,
        },
        MockMessage {
            id: 5,
            role: "user".to_string(),
            text_content: "That's really helpful! How does this compare to threads?".to_string(),
            created_at: now - 3400,
        },
        MockMessage {
            id: 6,
            role: "assistant".to_string(),
            text_content: "Great question! Async/await and threads solve different problems:\n\n**Async/await:**\n- Cooperative multitasking - tasks voluntarily yield control\n- Very lightweight - can have millions of tasks\n- Best for I/O-bound operations (network, file system, etc.)\n- Single-threaded by default (though runtimes can use multiple threads)\n- Lower memory overhead\n\n**Threads:**\n- Preemptive multitasking - OS controls switching\n- More expensive - limited to thousands of threads\n- Best for CPU-bound operations\n- True parallelism across CPU cores\n- Higher memory overhead\n\nFor most I/O operations like HTTP requests or database queries, async/await is more efficient because tasks can yield while waiting for I/O, allowing other tasks to run.".to_string(),
            created_at: now - 3350,
        },
        MockMessage {
            id: 7,
            role: "user".to_string(),
            text_content: "Perfect, that clarifies things! One more question - what are some common pitfalls to avoid?".to_string(),
            created_at: now - 3200,
        },
        MockMessage {
            id: 8,
            role: "assistant".to_string(),
            text_content: "Here are the most common pitfalls when working with async Rust:\n\n1. **Blocking in async functions**: Never call blocking operations like `std::thread::sleep` or blocking file I/O in async code. Use the async alternatives (e.g., `tokio::time::sleep`).\n\n2. **Forgetting .await**: Futures are lazy - they don't do anything until awaited or polled.\n\n3. **Send + Sync bounds**: When spawning tasks, make sure your types implement `Send` if they'll be moved across threads.\n\n4. **CPU-intensive work**: Don't run heavy computation in async tasks - it blocks the executor. Use `tokio::task::spawn_blocking` for CPU work.\n\n5. **Mixed runtimes**: Stick to one async runtime (tokio, async-std, etc.) - mixing them causes issues.\n\nWould you like me to show examples of how to avoid these pitfalls?".to_string(),
            created_at: now - 3100,
        },
        MockMessage {
            id: 9,
            role: "user".to_string(),
            text_content: "No, that's all I needed. Thanks so much for the detailed explanations!".to_string(),
            created_at: now - 2900,
        },
        MockMessage {
            id: 10,
            role: "assistant".to_string(),
            text_content: "You're very welcome! I'm glad I could help clarify async/await in Rust. Feel free to ask if you have more questions in the future. Happy coding!".to_string(),
            created_at: now - 2850,
        },
    ]
}

/// Generate mock search results
#[derive(Debug, Clone)]
pub struct MockSearchResult {
    pub conversation_id: i64,
    pub conversation_title: String,
    pub message_id: i64,
    pub role: String,
    pub snippet: String,
    pub created_at: i64,
}

pub fn mock_search_results() -> Vec<MockSearchResult> {
    let now = Utc::now().timestamp();

    vec![
        MockSearchResult {
            conversation_id: 1,
            conversation_title: "Rust async patterns".to_string(),
            message_id: 2,
            role: "assistant".to_string(),
            snippet: "...Async/await in **Rust** is a way to write asynchronous code that looks and behaves like synchronous code...".to_string(),
            created_at: now - 3590,
        },
        MockSearchResult {
            conversation_id: 3,
            conversation_title: "Debugging SQLite queries".to_string(),
            message_id: 15,
            role: "user".to_string(),
            snippet: "...How can I debug slow **Rust** SQLite queries?...".to_string(),
            created_at: now - 86200,
        },
        MockSearchResult {
            conversation_id: 7,
            conversation_title: "Error handling in Rust".to_string(),
            message_id: 8,
            role: "assistant".to_string(),
            snippet: "...In **Rust**, the Result type is the standard way to handle errors...".to_string(),
            created_at: now - 604500,
        },
    ]
}
