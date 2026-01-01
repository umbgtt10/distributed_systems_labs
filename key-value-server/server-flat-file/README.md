# Key-Value Server: Flat-File Implementation

File-based persistence implementation of the distributed key-value store using **JSON serialization** to a text file.

## Overview

This implementation stores all key-value pairs in a single JSON file (`storage.txt`), providing:
1. **Simple persistence** - Data survives server restarts
2. **Human-readable format** - Easy to inspect and debug
3. **Atomic writes** - File operations ensure consistency

It demonstrates how to implement the `Storage` trait for a file-based backend while handling the complexity of mixing synchronous file I/O with async gRPC operations.

## Implementation

```rust
pub struct FlatFileStorage {
    file_path: String,
}
```

**Storage format**: JSON file containing `HashMap<String, (String, u64)>`

```json
{
  "key1": ["value_123", 5],
  "key2": ["value_456", 3]
}
```

### Concurrency Model

- **File-level locking**: Each operation reads/writes the entire file
- **Trade-off**: Simple but limits concurrent throughput
- **Spawn blocking**: File I/O runs in `spawn_blocking` to avoid blocking the async runtime

### Atomic Operations

Each PUT operation:
1. Read entire file
2. Deserialize to HashMap
3. Modify in-memory
4. Serialize and write back

This provides **read-modify-write atomicity** at the file level, though it's not optimized for high concurrency.

## Building

```bash
cd key-value-server
cargo build --release --bin key-value-server-flat-file
```

## Running

```bash
cd key-value-server
cargo run --release --bin key-value-server-flat-file
```

The server:
1. Loads `config.json` from the key-value-server directory
2. Creates or opens `storage.txt` in the current directory
3. Spawns configured clients (default: 5 clients with overlapping keys)
4. Runs stress test for configured duration (default: 30 seconds)
5. Auto-shuts down and prints final storage state

**Note**: The `storage.txt` file persists between runs. Delete it to start fresh.

## Testing

Use the provided PowerShell script:

```powershell
cd server-flat-file\scripts
.\run_test.ps1
```

This script:
- Cleans up leftover processes
- Builds the server in release mode
- Runs a 30-second stress test
- Captures output and errors

## Storage File Location

The server creates `storage.txt` in **the directory where the executable runs** (typically `key-value-server\target\release\` when using `cargo run`).

To inspect the storage state:
```powershell
# After running the server
cat ..\target\release\storage.txt
```

Example output:
```json
{"key1":["value_3947693912",1],"key2":["value_2174462090",4]}
```

## Implementation Details

### Mixing Sync and Async

File I/O is inherently blocking, but the gRPC server is async. The solution:

```rust
async fn get(&self, key: &str) -> Result<(String, u64), StorageError> {
    let file_path = self.file_path.clone();
    let key = key.to_string();

    // Run blocking I/O in a thread pool
    spawn_blocking(move || {
        let file = File::open(&file_path)?;
        let data: HashMap<String, (String, u64)> = serde_json::from_reader(file)?;
        // ... rest of logic
    })
    .await
    .map_err(|e| /* handle JoinError */)?
}
```

**Why `spawn_blocking`**: Prevents file I/O from blocking the Tokio runtime's async executor.

### Error Handling

The implementation handles:
- **File not found**: Creates empty HashMap on first read
- **Corrupted JSON**: Returns storage errors
- **I/O errors**: Propagates as `StorageError`

### Persistence Guarantees

- **Durability**: Data survives process crashes (file is flushed on each write)
- **Atomicity**: Each operation is atomic at the file level
- **No transactions**: Multi-key operations are not atomic across keys

## Performance Characteristics

### Strengths
✅ Simple implementation (no external dependencies)
✅ Human-readable storage format
✅ Easy debugging (cat/grep the file)
✅ Predictable behavior

### Limitations
❌ **Not scalable**: Entire dataset loaded for every operation
❌ **Slow under concurrency**: Sequential file access
❌ **No crash recovery**: Partial writes can corrupt the file
❌ **Memory overhead**: Full dataset in memory during operations

### When to Use
- **Development/testing**: Simple persistence for demos
- **Small datasets**: Up to thousands of keys
- **Low concurrency**: Single-digit concurrent clients
- **Human inspection**: Need to debug storage state

### When NOT to Use
- Production workloads with high throughput
- Large datasets (> 10K keys)
- High concurrency (> 10 concurrent clients)
- Critical data requiring ACID guarantees

## Comparison with Other Implementations

| Feature | In-Memory | Flat-File | Sled DB |
|---------|-----------|-----------|---------|
| Persistence | ❌ None | ✅ Full | ✅ Full + ACID |
| Throughput | Highest | Low | High |
| Latency | ~1ms | ~10-50ms | ~1-5ms |
| Concurrency | Mutex | File lock | Lock-free |
| Debugging | Memory dump | Cat file | CLI tool |
| Dependencies | None | None | Sled crate |

## Improvements

Potential enhancements (not implemented):
1. **Write-ahead log**: Append-only log for crash recovery
2. **Memory mapping**: Avoid full serialization on each operation
3. **Read caching**: Cache in-memory, write-through to disk
4. **Compaction**: Periodic file reorganization
5. **Multiple files**: Shard keys across files for concurrency

These improvements would move toward a real embedded database (like Sled).

## Code Structure

```
server-flat-file/
├── src/
│   ├── main.rs              # Server startup and runner
│   └── flat_file_storage.rs # Storage trait implementation
├── scripts/
│   └── run_test.ps1         # Stress test script
└── Cargo.toml
```

The implementation is intentionally kept simple (~100 lines) to demonstrate the core concepts without over-engineering.

## Next Steps

1. **Run the test**: `.\scripts\run_test.ps1` to see it in action
2. **Inspect storage**: Examine `storage.txt` to understand the format
3. **Compare**: Run the in-memory and Sled implementations to see performance differences
4. **Extend**: Try implementing write-ahead logging or caching

---

**[← Back to Key-Value Server](../README.md)**
