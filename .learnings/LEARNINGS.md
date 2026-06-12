# Learnings

Corrections, insights, and knowledge gaps captured during development.

**Categories**: correction | insight | knowledge_gap | best_practice

---

## [LRN-20260612-001] correction

**Logged**: 2026-06-12T00:00:00Z
**Priority**: high
**Status**: promoted
**Area**: config

### Summary
Release version consistency must include `src-tauri/tauri.conf.json`, not only `package.json`, `Cargo.toml`, and `Cargo.lock`.

### Details
After overwriting `v3.16.4`, the app About page still showed `v3.16.2`. Investigation confirmed `src-tauri/tauri.conf.json` remained `"version": "3.16.2"`; Tauri/macOS App metadata and About page use this value, while update detection reads GitHub tag/release.

### Suggested Action
Before any release tag push or overwrite, verify `package.json`, `src-tauri/Cargo.toml`, `src-tauri/Cargo.lock`, and `src-tauri/tauri.conf.json` all match the target tag version.

### Metadata
- Source: user_feedback
- Related Files: src-tauri/tauri.conf.json, package.json, src-tauri/Cargo.toml, src-tauri/Cargo.lock
- Tags: release, tauri, version-consistency
- Promoted: project memory release_version_consistency_checklist.md

---
