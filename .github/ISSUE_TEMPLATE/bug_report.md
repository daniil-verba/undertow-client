---
name: 🐞 Bug Report
about: Report a problem or unexpected behavior
title: '[BUG] Short description'
labels: bug
assignees: ''
---

## Description
Clearly describe what happened.

## Steps to Reproduce
1. Run `cargo run`
2. Send message to `peer_id`
3. Observe error...

## Expected Behavior
What should have happened?

## Actual Behavior
What actually happened?

## Environment
- OS: Arch Linux
- Rust Version: 1.70.0
- Protocol Version: v0.1.0

## Logs (if available)
```
[2026-07-16 10:00:00] ERROR: connection timeout
```

## Possible Solution (optional)
Try increasing timeout in `network/relay.rs`.

## Additional Context
- Related Issues: #42
```
