# MongoDB Copy CLI

A powerful command-line tool for copying MongoDB databases and collections between instances with interactive selection and flexible options.

## Features

- [x] Copy entire databases or specific collections
- [x] Interactive selection with multi-select support
- [x] Sample data copying (copy only N documents)
- [x] Rename databases/collections during copy
- [x] Display document counts before copying
- [x] Batch processing for efficient data transfer
- [x] Connection URI masking for security
- [x] Environment variable support
- [x] Progress indicators during copy operations

## Installation

### Prerequisites

- Rust toolchain (1.70+)
- Access to source and destination MongoDB instances

### Build from source

```bash
cargo build --release
```

The binary will be available at `target/release/mongo-copy`

## Usage

### Basic Usage

Run the CLI without arguments to use interactive prompts:

```bash
cargo run --release
```

Or use the compiled binary:

```bash
./target/release/mongo-copy
```

### Using Environment Variables

Set environment variables to avoid entering URIs each time:

```bash
export MONGODB_URI_SOURCE="mongodb://localhost:27017"
export MONGODB_URI_DESTINATION="mongodb://localhost:27018"
cargo run --release
```

### Using Command-Line Arguments

Override environment variables with command-line arguments:

```bash
cargo run --release -- --source "mongodb://localhost:27017" --destination "mongodb://localhost:27018"
```

## Workflow

### 1. Connection

The tool will:

- Check for `MONGODB_URI_SOURCE` environment variable or prompt for source URI
- Check for `MONGODB_URI_DESTINATION` environment variable or prompt for destination URI
- Display masked URIs (credentials hidden)
- Test connections to both instances

### 2. Copy Mode Selection

Choose between:

- **Copy entire database(s)**: Copy all collections from selected databases
- **Copy specific collection(s)**: Copy individual collections with more control

### 3. Database Copy Mode

When copying databases:

1. Select one or more databases from the source (multi-select with space bar)
2. For each database, choose to keep the same name or rename it
3. Confirm the operation
4. All collections in the database will be copied

### 4. Collection Copy Mode

When copying collections:

1. Select the source database
2. Select one or more collections (multi-select with space bar)
3. For each collection:
   - View the estimated document count
   - Choose to copy all documents or specify a limit (sample)
   - Choose to keep the same database name or rename it
   - Choose to keep the same collection name or rename it
   - Confirm the operation
4. Documents are copied in batches of 1000 for efficiency

## Interactive Controls

- **Space**: Select/deselect items in multi-select lists
- **Enter**: Confirm selection
- **↑/↓**: Navigate through options
- **Esc**: Cancel operation
- **Type**: Filter options in select lists

## Connection String Format

MongoDB connection strings follow the standard format:

```
mongodb://[username:password@]host[:port][/database][?options]
```

Examples:

- `mongodb://localhost:27017`
- `mongodb://user:pass@localhost:27017`
- `mongodb://user:pass@host1:27017,host2:27017/mydb?replicaSet=rs0`
- `mongodb+srv://user:pass@cluster.mongodb.net/mydb`

## Performance

- Documents are copied in batches of 1000 for optimal performance
- Progress is displayed every 1000 documents
- Uses MongoDB's native drivers for efficient data transfer
- Estimated document counts are used (fast but approximate)

## Security

- Connection URIs are masked in output (credentials hidden)
- No credentials are logged or stored
- Direct connection between source and destination
- All data transfer happens through the CLI process

## Error Handling

The tool will:

- Validate connection strings before attempting to connect
- Test connections before starting copy operations
- Display clear error messages for connection failures
- Allow you to skip operations if confirmation is declined
- Handle network interruptions gracefully

## Limitations

- Large collections may take significant time to copy
- No incremental/differential copy support
- No automatic index copying (indexes must be recreated manually)
- No schema validation during copy
- Requires network connectivity to both MongoDB instances

## Dependencies

- `clap` - Command-line argument parsing
- `mongodb` - Official MongoDB Rust driver
- `inquire` - Interactive CLI prompts
- `tokio` - Async runtime
- `anyhow` - Error handling
- `futures` - Async utilities

## License

This project is provided as-is for educational and practical use.

## Contributing

Contributions are welcome! Please feel free to submit issues or pull requests.
