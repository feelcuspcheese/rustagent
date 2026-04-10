# Rust Appointment Agent

A Rust-based agent for monitoring library passes (LibCal-based systems) and either sending ntfy notifications or booking appointments automatically.

## Features

- **Dual Mode Operation**: Alert mode (sends notifications) or Booking mode (automatically books appointments)
- **Multi-Site Support**: Configurable for multiple library systems (SPL, KCLS, etc.)
- **Stealth Operations**: Chrome 124 browser impersonation with proper TLS fingerprinting
- **Smart Scheduling**: Precision timing with pre-warming, jitter, and rest cycles
- **Real-time Dashboard**: Web interface with WebSocket log streaming
- **Failure Handling**: Automatic retries with exponential backoff

## Project Structure

```
appointment-agent/
├── Cargo.toml              # Dependencies and build configuration
├── Dockerfile              # Multi-stage Docker build
├── entrypoint.sh           # Docker entrypoint script
├── configs/
│   └── default_config.yaml # Default configuration
├── src/
│   ├── main.rs             # Application entry point
│   ├── config.rs           # Configuration loading and structures
│   ├── client_pool.rs      # HTTP client pool with rotation
│   ├── scraper.rs          # HTML parsing and availability detection
│   ├── booker.rs           # Login and booking operations
│   ├── agent.rs            # Main agent orchestration engine
│   └── web/
│       └── mod.rs          # Web dashboard and API
└── tests/
    └── integration_tests.rs # Integration tests
```

## Configuration

The agent is configured via a YAML file (`configs/config.yaml`):

```yaml
active_site: spl
mode: alert  # or "booking"
preferred_days: ["Saturday", "Sunday"]
strike_time: "09:00"
check_window: 60s
ntfy_topic: myappointments

credentials:
  my_card:
    name: "My Library Card"
    username: "123456"
    password: "PIN"
    email: "me@example.com"
    site: spl

sites:
  spl:
    name: "Seattle Public Library"
    baseurl: "https://spl.libcal.com"
    # ... additional site configuration
```

## Usage

### Running Locally

```bash
# Run in CLI mode
cargo run -- --config configs/config.yaml

# Run with web dashboard
cargo run -- --config configs/config.yaml --web

# Dry-run mode (no actual bookings)
cargo run -- --config configs/config.yaml --dry-run
```

### Running with Docker

```bash
# Build the image
docker build -t appointment-agent .

# Run the container
docker run -d \
  -p 8080:8080 \
  -v $(pwd)/configs:/root/configs \
  appointment-agent
```

## API Endpoints

When running in web mode, the following endpoints are available:

- `GET /api/config` - Get current configuration
- `PUT /api/config` - Update configuration
- `GET /api/runs` - Get run history
- `GET /api/logs` - Get recent logs
- `GET /api/ws` - WebSocket endpoint for real-time logs

## Logging Contract

The agent emits JSON logs with the following events:

- `agent_start` - Agent run started
- `pre_warm_start` / `pre_warm_complete` - Pre-warming phase
- `check_window_start` - Check window opened
- `availability_found` / `no_availability` - Availability status
- `login_required` / `login_success` - Login events
- `booking_attempt` / `booking_success` / `booking_failed` - Booking events
- `notification_sent` - Notification sent to ntfy
- `check_window_expired` - Check window closed
- `agent_finished` - Agent run completed

## Testing

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_config_parsing

# Run integration tests
cargo test --test integration_tests
```

## Requirements

- Rust 1.85+
- Docker (optional, for containerized deployment)

## License

MIT
