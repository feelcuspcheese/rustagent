Rust Appointment Agent: Complete Agent‑Actionable Documentation
Version 2.0 – Agent‑Ready (April 2026)
This document is not just architecture — it is a direct execution plan for an AI agent. It contains target specifications, real config examples, mapping rules, sample HTML, failure handling, and a logging contract. An agent reading this can autonomously build and run the system without guessing.

1. Executive Summary (Agent Perspective)
Your goal: implement a Rust program that monitors library passes (LibCal‑based systems) and either sends ntfy notifications or books appointments automatically.

You are given:

A config.yaml that defines sites (SPL, KCLS) with museums, endpoints, selectors, and login/booking forms.

A set of execution rules (how to construct URLs, parse HTML, detect login, submit forms).

Failure handling and logging requirements.

You must NOT guess – all runtime behavior is derived from the config.

2. Target Execution Contract (Mandatory for Agent)
2.1 What the Agent Receives (Config)
A YAML file with the following structure. All fields are mandatory unless marked optional.

yaml
active_site: spl                     # which site is currently selected
mode: alert                          # "alert" or "booking"
preferred_days: ["Saturday", "Sunday"]
strike_time: "09:00"
check_window: 60s
check_interval: 2s
pre_warm_offset: 30s
request_jitter: 2s
months_to_check: 2
rest_cycle_checks: 20
rest_cycle_duration: 3s
ntfy_topic: myappointments

credentials:
  my_card:
    name: "My Library Card"
    username: "123456"
    password: "PIN"
    email: "me@example.com"
    site: spl

selected_credential: my_card

sites:
  spl:
    name: "Seattle Public Library"
    baseurl: "https://spl.libcal.com"
    availabilityendpoint: "/pass/availability/institution"
    digital: true
    physical: false
    location: "0"
    bookinglinkselector: "a.s-lc-pass-availability.s-lc-pass-digital.s-lc-pass-available"
    successindicator: "Thank you!"
    loginform:
      usernamefield: "username"
      passwordfield: "password"
      submitbutton: "submit"
      csrfselector: ""            # optional
      authidselector: "input[name='auth_id']"
      loginurlselector: "input[name='login_url']"
    bookingform:
      actionurl: ""               # optional, extracted from page
      emailfield: "email"
    museums:
      sam:
        name: "Seattle Art Museum"
        slug: "SAM"
        museumid: "7f2ac5c414b2"
    preferredslug: "sam"

  kcls:
    name: "King County Library System"
    baseurl: "https://rooms.kcls.org"
    availabilityendpoint: "/pass/availability/institution"
    digital: true
    physical: false
    location: "0"
    bookinglinkselector: "a.s-lc-pass-availability.s-lc-pass-digital.s-lc-pass-available"
    successindicator: "Thank you!"
    loginform:
      usernamefield: "username"
      passwordfield: "password"
      submitbutton: "submit"
      csrfselector: ""
      authidselector: "input[name='auth_id']"
      loginurlselector: "input[name='login_url']"
    bookingform:
      actionurl: ""
      emailfield: "email"
    museums:
      kidsquest:
        name: "KidsQuest Children's Museum"
        slug: "kidsquest"
        museumid: "9ec25160a8a0"
    preferredslug: "kidsquest"
2.2 URL Construction Rules
The agent MUST construct URLs as follows:

Availability URL

text
{baseurl}{availabilityendpoint}?museum={museumid}&date={YYYY-MM-DD}&digital={digital}&physical={physical}&location={location}
Example:
https://spl.libcal.com/pass/availability/institution?museum=7f2ac5c414b2&date=2026-04-15&digital=true&physical=false&location=0

Booking URL – from href of an available slot (relative or absolute). If relative, prepend {baseurl}.

Login action URL – extracted from the login form’s action attribute. If relative, resolve against the current page’s base URL.

Booking form action URL – extracted from the booking form’s action attribute. If relative, resolve similarly.

2.3 Selector & Parsing Rules
Availability parsing:

Load HTML of the availability response.

Use CSS selector bookinglinkselector.

For each matching element:

Extract href → booking_url

Extract inner text → date (the day number; later we combine with the requested month/year to get full date).

Login form detection:

Search for <form action*="form_login"> (any form whose action contains “form_login”).

Inside that form, extract hidden inputs:

authidselector → auth_id

loginurlselector → login_url

Also extract the form’s action URL.

Booking form detection:

After successful login, look for <form id="s-lc-bform">.

Extract all <input type="hidden"> fields (name/value).

Also look for the email field (if present) – use bookingform.emailfield to set its value.

2.4 Execution Flow (Agent Decision Engine)
2.4.1 Precision Wait
Wait until drop_time - pre_warm_offset (pre‑warm phase).

Perform a GET to baseurl (this establishes TLS session and cookies).

Wait until exact drop_time using a spin‑loop for the last 10ms.

2.4.2 Check Window Loop
Loop until now > drop_time + check_window.

Before each check, apply jitter: sleep for random [0, request_jitter].

Fetch availability for the month containing drop_time and the next months_to_check - 1 months (concurrently, with a semaphore limit of 3).

For each fetched availability, extract the full date from the booking URL (query parameter date).

If date not in seen_dates, add it and process.

2.4.3 Alert Mode
Build notification: up to 3 actions (weekends first), message with emojis and dates.

Send ntfy notification.

Stop the run.

2.4.4 Booking Mode
For each new date in preferred day order:

Attempt to book.

If booking succeeds, stop the run.

If booking fails, continue to next date.

2.4.5 Rest Cycle
If check_window > 1 minute, after every rest_cycle_checks checks, sleep for rest_cycle_duration.

2.5 Login & Booking Rules
Login:

Follow the booking URL (the agent follows redirects automatically).

If the page contains a login form (detected by action*="form_login"), extract auth_id and login_url.

POST to the login action with fields:

auth_id = extracted value

login_url = extracted value

username = from selected credential

password = from selected credential

After login, the client will be redirected (cookies saved). Follow to the booking page.

Booking:

On the booking page, find form#s-lc-bform.

Collect all hidden input values.

Add the email field (value from credential).

POST to the form’s action URL.

Check the response HTML for successindicator (e.g., "Thank you!").

2.6 Failure Handling
Failure	Action
Selector returns no elements	Log warning, try a relaxed selector (e.g., "a"), then continue loop.
Login returns 4xx/5xx	Retry up to 2 times, with same client (cookies persist). After 2 failures, abort the run.
Booking form not found	Log error with response snippet, abort booking for this date.
HTTP timeout	Retry once after 2 seconds.
Network error	Log and retry after 5 seconds (up to 3 times).
2.7 Logging Contract (Required Output)
The agent MUST emit JSON logs (one per line) at INFO level, with at least these events:

json
{"event": "agent_start", "run_id": "...", "drop_time": "..."}
{"event": "pre_warm_start"}
{"event": "pre_warm_complete"}
{"event": "check_window_start", "deadline": "..."}
{"event": "availability_fetch", "url": "...", "month": "..."}
{"event": "availability_found", "date": "...", "booking_url": "..."}
{"event": "no_availability", "date": "..."}
{"event": "login_required"}
{"event": "login_success"}
{"event": "booking_attempt", "date": "..."}
{"event": "booking_success", "date": "..."}
{"event": "booking_failed", "date": "...", "error": "..."}
{"event": "notification_sent", "title": "...", "topic": "..."}
{"event": "check_window_expired"}
{"event": "agent_finished"}
These logs are sent to the WebSocket clients and also printed to stdout.

2.8 Stealth Requirements
TLS fingerprinting: The HTTP client must impersonate a real browser (Chrome 124). Use wreq with Browser::Chrome124.

Header order: Must preserve case and order as the browser would. Use wreq default headers.

Jitter: Random delay before each availability request.

Rest cycles: Human‑like pauses after many rapid checks.

User‑Agent rotation: Not required (single profile per run), but the client pool can rotate profiles for different runs.

2.9 Sample HTML (Reality Anchors)
To avoid hallucinations, here are exact HTML snippets the agent will encounter.

Availability calendar (abbreviated):

html
<div class="s-lc-pass-calendar by-museum">
  <div class="day day-Wed day-2026-04-15">
    <a href="/passes/SAM/book?date=2026-04-15&pass=abc123" class="s-lc-pass-availability s-lc-pass-digital s-lc-pass-available">
      15
    </a>
  </div>
</div>
Login form (after clicking a book link):

html
<form action="https://libauth.com/form_login" method="post">
  <input type="hidden" name="auth_id" value="3001">
  <input type="hidden" name="login_url" value="https://spl.libapps.com/libapps/libauth?auth_id=3001">
  <input type="text" name="username">
  <input type="password" name="password">
  <button type="submit">Login</button>
</form>
Booking form (after login):

html
<form id="s-lc-bform" method="post" action="/passes/7f2ac5c414b2/book">
  <input type="hidden" name="museum" value="7f2ac5c414b2">
  <input type="hidden" name="pass" value="63ea409f9fab">
  <input type="hidden" name="date" value="2026-04-15">
  <input type="hidden" name="crc" value="d50ab18d9fa26567aaee1106311fa618">
  <div class="form-group">
    <input type="email" name="email" required>
  </div>
  <button type="submit">Submit my Booking</button>
</form>
Success page:

html
<h1 id="s-lc-eq-success-title">Thank you!</h1>
<p>The following Digital Pass reservation was made:</p>
3. Implementation Guide (Agent‑Friendly)
3.1 Project Setup
bash
cargo new appointment-agent
cd appointment-agent
Replace Cargo.toml with the content from the appendix.

3.2 Core Modules (Code Structure)
The agent must implement the following files exactly as described.

src/config.rs – load/save config using serde_yml
(Code provided in earlier version; ensure it matches the YAML structure above.)

src/client_pool.rs – HTTP client with rotation
rust
use wreq::{Client, Browser};
use std::sync::Arc;

pub struct ClientPool {
    clients: Vec<Arc<Client>>,
}

impl ClientPool {
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            .impersonate(Browser::Chrome124)
            .cookie_store(true)
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        Ok(Self { clients: vec![Arc::new(client)] })
    }
    pub fn next(&self) -> &Client { &self.clients[0] }
}
src/scraper.rs – fetch and parse availability
(Use the selector and URL construction rules.)

src/booker.rs – login and booking
(Follow the login detection and form submission rules exactly.)

src/agent.rs – the orchestration engine
(Implement the decision loop, precision wait, rest cycle, and logging contract.)

src/web/ – dashboard with axum and WebSocket logs
(Implement the API endpoints: /api/config, /api/runs, /api/logs, etc.)

3.3 Testing
Unit tests: for config parsing, URL construction, selector extraction.

Integration test with a mock server (using wiremock) that returns the sample HTML snippets.

Stealth test: call https://tls.peet.ws/api/all and verify JA3 fingerprint matches Chrome 124.

4. Appendix: Complete Cargo.toml and Dockerfile
4.1 Cargo.toml
toml
[package]
name = "appointment-agent"
version = "0.1.0"
edition = "2024"

[dependencies]
tokio = { version = "1.50", features = ["full", "tracing", "signal"] }
wreq = "6.0.0-rc.26"
wreq-util = "3.0.0-rc.1"
boring-sys = "4.15.0"
scraper = "0.21"
axum = "0.8"
tokio-tungstenite = "0.28"
tower = "0.5"
tower-http = { version = "0.6", features = ["cors", "trace", "fs"] }
rust-embed = "8.5"
serde = { version = "1.0", features = ["derive"] }
serde_yml = "0.3"
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
dashmap = "6.1"
uuid = { version = "1.12", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
anyhow = "1.0"
thiserror = "2.0"
rand = "0.8"
url = "2.5"
clap = { version = "4.5", features = ["derive"] }
humantime = "2.1"

[dev-dependencies]
wiremock = "0.6"

[profile.release]
lto = true
codegen-units = 1
opt-level = 3
4.2 Dockerfile (multi‑stage with musl)
dockerfile
FROM rust:1.85-slim-bookworm AS builder
RUN apt-get update && apt-get install -y \
    cmake ninja-build pkg-config perl libclang-dev musl-tools \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
RUN cargo fetch
COPY . .
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM alpine:latest
RUN apk add --no-cache ca-certificates
WORKDIR /root/
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/appointment-agent /usr/local/bin/agent
COPY configs/default_config.yaml /root/configs/default_config.yaml
COPY entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh
EXPOSE 8080
ENTRYPOINT ["/entrypoint.sh"]
4.3 entrypoint.sh
bash
#!/bin/sh
if [ ! -s /root/configs/config.yaml ]; then
    cp /root/configs/default_config.yaml /root/configs/config.yaml
fi
exec /usr/local/bin/agent --web
5. Final Agent Instructions
Read the target specification – all runtime data is in config.yaml.

Follow the URL construction, selector, login, and booking rules exactly – do not invent.

Emit the required JSON logs – they are used by the dashboard.

Use the provided HTML snippets as reality anchors – if the actual page differs, adapt but keep the same logic.

Handle failures as defined – retry, rotate, or abort.

Build with the given Dockerfile and Cargo.toml – they are pre‑verified.
