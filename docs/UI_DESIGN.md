# rust-mule UI Design Specification

**Chat-style Search Dashboard**

## Overview

The rust-mule UI adopts a ChatGPT-like layout, where:

- the left sidebar contains navigation and a list of keyword searches
- the main panel displays the active search’s overview, progress, and results
- each keyword search behaves like a “chat thread”, persisting over time

This UI is designed to be:

- lightweight (HTML + Alpine.js)
- embeddable into the rust-mule binary
- usable locally first, headless-friendly later
- suitable for long-running / overnight operations

## Goals

- Familiar interaction model inspired by OpenAI Chat UI
- Keyword searches as first-class, persistent entities
- Fast navigation between searches
- Live progress and telemetry
- Minimal frontend complexity (no SPA framework, no build step)

## Non-Goals (v1)

- Multi-user accounts
- Cloud sync
- Full download management UI
- Complex routing or state frameworks
- Visual polish beyond clarity and usability

## Global Layout

Application Shell (Persistent):

+--------------------------------------------------+
| Sidebar | Main Content |
|--------------------------|-----------------------|
| Primary Nav | Header |
| - Overview | Search Overview |
| - Searches | KPIs |
| - Nodes / Routing | Progress Graph |
| - Logs | Results |
| - Settings | Activity / Logs |
| | |
| Search List | |
| - keyword A (RUNNING) | |
| - keyword B (DONE) | |
| - keyword C (ERROR) | |
| | |
| + New Search | |
+--------------------------------------------------+

## Sidebar Specification

### Primary Navigation

- Overview
- Searches
- Nodes / Routing
- Logs
- Settings

### Search List

Each entry represents a keyword search.

- keyword (title)
- state badge: idle | running | complete | error
- optional secondary info: hits, last updated, peers

### New Search Control

- - New search button
- keyword input
- optional filters (later)

## Core Domain Concept: Search

Search properties:

- id
- keyword
- state
- created_at
- last_updated_at
- progress
- telemetry snapshot
- results summary
- event/log history (optional)

Search lifecycle:

1. Created
2. Started
3. Running
4. Completed / Error / Stopped
5. May be re-run

## Main Panel: Search Overview

### Header

- keyword
- state badge
- actions: start, stop, export, delete

### KPIs

- peers contacted
- requests sent / responses
- hits found
- download candidates

### Progress Visualization

- line graph over time
- minimal v1: counters + sparkline

### Results

- file name
- size
- sources
- confidence
- actions (download later)

### Activity / Logs

- per-search log tail
- last 200–500 lines

## Overview Page

- new search input
- recent searches
- global system health
- morning report summary

## Live Data & Updates

- GET /api/v1/searches
- GET /api/v1/searches/:id
- GET /api/v1/events (SSE)

Event types:

- search_created
- search_state_changed
- search_progress
- search_stats
- search_hit_found
- search_completed
- log

## Navigation Model

- shared shell replicated across pages (`index`, `search`, `search_details`, `node_stats`, `log`, `settings`)
- sidebar switches pages and links to active search details
- URL params: `?searchId=<id>` on `search_details`

## UI Components

Sidebar:

- nav item
- search item
- state badge
- new search input

Main:

- header
- KPI cards
- chart
- results table
- log viewer

## UX Principles

- searches feel like conversations
- running state is obvious
- overnight usage is first-class
- clarity over density

## API Contract (v1)

- GET /api/v1/searches
- GET /api/v1/searches/:id
- POST /api/v1/searches/:id/stop
- DELETE /api/v1/searches/:id
- POST /api/v1/kad/search_keyword
- GET /api/v1/kad/keyword_results/:keyword_id_hex
- GET /api/v1/events

## Implementation Snapshot (2026-02-12)

- Implemented: shell/sidebar pages, search form, search thread list, search details, node status page, logs page, settings page, theme selector.
- Implemented: UI files embedded in binary and served by Rust API.
- Partial: statistics charting (Chart.js bundled, chart views not yet implemented).
- Open: API-backed settings CRUD (`GET/PATCH /api/settings`) and token handling for SSE without query parameter exposure.

## Implementation Notes

- Alpine.js app shell
- SSE for live updates
- CSS variables for theming
- minimal JS, no framework gravity

## v1 Milestones

1. Shell + sidebar
2. Create searches
3. Search overview
4. Live updates
5. Logs per search
6. Progress graph
7. Results list

## Guiding Principle

Make searches feel like conversations with the network.
