# SHC Eco Simulator

SHC Eco Simulator is a Rust workspace for building an economics/planning simulator inspired by **Stronghold Crusader: Definitive Edition**.

The goal is to model economy layouts and building placement on a cell grid, then evaluate which setups are most effective.

This repository has two crates:

- `simulator`: core simulation/domain logic (map, buildings, placement rules, game time, simulation state)
- `gui`: Slint desktop UI for map editing, scenario setup, simulation control, and later charts/plots

## Current Status

This is an early foundation build.

Implemented now:

- Workspace with `simulator` and `gui`
- Square cell map model
- Building types and placement rules
- Occupancy checks (cannot place on occupied cells)
- Basic map editor UI with building palette and zoom controls

Planned next:

- Production chains and resource flow
- Worker travel and throughput logic
- Time-step simulation loop
- Plot/graph output after simulation runs
- Save/load and scenario persistence

## Product Vision

The simulator is intended to answer practical questions such as:

- Which eco layout gives best throughput for a target good?
- How much does placement distance reduce output?
- Which workshop/stockyard/armoury arrangement is best under map constraints?

The UI should let users quickly paint layouts, run simulation steps, and inspect metrics visually.

## Domain Baseline

### Map

- Cell-based square map
- User-defined size `N x N`
- Default size: `100`

### Building Placement

- Buildings are placed by bottom-left cell `(x, y)`
- Footprint is square/templated per building type
- A placement occupies specific map cells
- Placement is rejected if any required cell is out of bounds or already occupied

### Initial Building Types

- `GoodsYard`
- `Armoury`
- `FletchersWorkshop`
- `BlacksmithsWorkshop`
- `PoleturnersWorkshop`
- `ArmourersWorkshop`

Workshops and armoury currently use a 4x4 square footprint.

`GoodsYard` uses a 5x5 pattern with four 2x2 corner stocks and a free center row/column cross.

## Architecture

## Workspace

- root `Cargo.toml` defines workspace members
- each crate owns its own concerns

## `simulator` crate structure

- `buildings/`
  - building definitions and placement data
  - factory/helpers to instantiate placements
- `map/`
  - cell map representation
  - occupancy and placement validation
- `simulation/`
  - game time and high-level simulation state/entry point

Design principles:

- deterministic behavior
- simple APIs
- explicit data flow
- low complexity over clever abstractions

## `gui` crate structure

- `ui/` Slint UI files
- `src/` Rust UI integration and controller logic
- `assets/` building icons for placement palette

GUI responsibilities:

- map editor interactions
- tool/building selection
- viewport zoom/pan (pan can be added next)
- invoking simulator APIs and reflecting state

## Build and Run

From repository root:

```powershell
cargo check
cargo test -p simulator
cargo run -p gui
```

Release build for GUI:

```powershell
cargo build --release -p gui
```

Windows release executable:

- `target/release/gui.exe`

## Coding Standards

Project coding standards are in:

- `skills/rust.md`

Highlights:

- prefer readability and simple control flow
- avoid unnecessary complexity/abstractions
- no line-by-line comments; comment only non-obvious intent
- structure code by responsibility

## Notes

This repository intentionally starts with a clean, modular base so we can iterate quickly on simulation mechanics and UI tooling.
