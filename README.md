# SHC Eco Simulator

SHC Eco Simulator is a Rust workspace for building an economics/planning simulator inspired by **Stronghold Crusader: Definitive Edition**.

The goal is to model economy layouts and building placement on a cell grid, then evaluate which setups are most effective.

This repository has two crates:

- `simulator`: core simulation/domain logic (map, buildings, placement rules, game time, simulation state)
- `gui`: Slint desktop UI for map editing, scenario setup, simulation control, and later charts/plots

## Table of Contents

- [Current Status](#current-status)
- [Product Vision](#product-vision)
- [Documentation](#documentation)
- [Project Files](#project-files)
- [Domain Baseline](#domain-baseline)
  - [Map](#map)
  - [Building Placement](#building-placement)
  - [Initial Building Types](#initial-building-types)
  - [Building Costs and Build Resources](#building-costs-and-build-resources)
  - [Goods Buy/Sell Costs](#goods-buysell-costs)
  - [Weapon Production and Sale](#weapon-production-and-sale)
  - [Production Cycle Model](#production-cycle-model)
  - [Wall Object](#wall-object)
  - [Worker Speed](#worker-speed)
  - [Distance Objects](#distance-objects)
- [Architecture](#architecture)
  - [Workspace](#workspace)
  - [`simulator` crate structure](#simulator-crate-structure)
  - [`gui` crate structure](#gui-crate-structure)
- [Build and Run](#build-and-run)
- [Coding Standards](#coding-standards)
- [Notes](#notes)

## Current Status

This is an early foundation build.

Implemented now:

- Workspace with `simulator` and `gui`
- Square cell map model
- Building types and placement rules
- Occupancy checks (cannot place on occupied cells)
- Basic map editor UI with building palette and zoom controls
- Portable JSON project save/load, Save As, and latest-recent-file support

Planned next:

- Production chains and resource flow
- Worker travel and throughput logic
- Time-step simulation loop
- Plot/graph output after simulation runs

## Product Vision

The simulator is intended to answer practical questions such as:

- Which eco layout gives best throughput for a target good?
- How much does placement distance reduce output?
- Which workshop/stockyard/armoury arrangement is best under map constraints?

The UI should let users quickly paint layouts, run simulation steps, and inspect metrics visually.

## Documentation

Additional domain notes live in `doc/`.

- [Unit movement speed](doc/unit_movement_speed.md) documents the tick formula, unit slowdown and speed-up coefficients, terrain slowdown modifiers, quick per-cell tick references, and a requested army-unit reference set with movement timings and damage data.

## Project Files

- `Open`, `Open Recent`, `Save`, and `Save As` are available above the map. Keyboard shortcuts are `Ctrl+O` and `Ctrl+S`.
- New saves default to `%LOCALAPPDATA%\SHCEcoSimulator\Projects` on Windows, but the standard file dialog can save or open a shared `.json` file anywhere.
- A project stores map geometry, building IDs and positions, goods-yard groups, stockpile resources, entry points, walls, and simulation settings such as selected weapons and resource-buy options.
- Calculated paths, worker distances, production results, and tooltip statistics are not stored; they are regenerated after opening.
- The recent-file list is stored in `%LOCALAPPDATA%\SHCEcoSimulator\recent.json`.

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
- Buildings now carry an optional `entry_point` field (`None` when no access is available)
- Placing `GoodsYard` creates four independent `Stockpile` buildings (2x2 each), grouped under one goods-yard group id.
- Each `Stockpile` has its own entry point and can be connected independently.
- Removing any stockpile from a goods-yard group removes all four stockpiles from that group.
- Entry point assignment is automatic on placement:
  - default: `(x + floor(n/2), y - 1)` where `x,y` is bottom-left and `n` is size
  - special case: when `n == 2`, default is `(x, y - 1)`
  - if default is blocked, candidate side-neighbor cells of external square are checked clockwise (corners excluded)
  - if side-neighbor cells are blocked, corners are checked clockwise starting from `(x + n, y - 1)`
  - if no candidate is free/in-bounds, entry point remains `None`

### Initial Building Types

- `GoodsYard`
- `Stockpile` (created automatically from Goods Yard placement)
- `Armoury`
- `FletchersWorkshop`
- `BlacksmithsWorkshop`
- `PoleturnersWorkshop`
- `ArmourersWorkshop`
- `WheatFarm`
- `Windmill`
- `Bakery`
- `Granary`

Workshops and armoury currently use a 4x4 square footprint.

Bread economy footprints:

- Wheat Farm: 9x9 reserved area. Its 3x3 top-left cabin blocks paths; field cells are traversable.
- Wind Mill: 3x3 with a fixed bottom-center entry.
- Bakery: 4x4 and uses the same wall-controlled entry orientation as weapon workshops.
- Granary: 4x4 with the same entry behavior as an Armoury.

`GoodsYard` placement uses a 5x5 template area with four 2x2 corner stockpiles and a free center row/column cross.

### Building Costs and Build Resources

`0` means this resource is not needed.

| Building               | Wood Cost | Gold Cost |
| ---------------------- | --------- | --------- |
| Goods Yard             |    0      |     0     |
| Stockpile              |    0      |     0     |
| Armoury                |    5      |     0     |
| Fletchers Workshop     |    20     |     100   |
| Blacksmiths Workshop   |    20     |     200   |
| Poleturners Workshop   |    10     |     100   |
| Armourers Workshop     |    20     |     100   |
| Wheat Farm             |    15     |     0     |
| Wind Mill              |    20     |     0     |
| Bakery                 |    10     |     0     |
| Granary                |    5      |     0     |

### Goods Buy/Sell Costs

| Goods | Sell | Buy |
| ----- | ---- | --- |
| Wood  |  1   |  4  |
| Iron  |  23  | 45  |
| Wheat |  8   | 23  |
| Flour |  10  | 32  |
| Bread |  4   |  8  |

### Weapon Production and Sale

Empty cell means this resource is not needed.

Worker can carry only one resource unit from stockpile per trip.
If a weapon needs `2` wood or `2` iron, that means two separate stockpile-to-workshop paths/trips.

| Weapon   | Workshop              | Wood Req | Iron Req | Make Time (Ticks) | Sell Gold |
| -------- | --------------------- | -------- | -------- | ----------------- | --------- |
| Bow      | Fletchers Workshop    |    2     |          |        638        |     15    |
| Crossbow | Fletchers Workshop    |    3     |          |        565        |     30    |
| Spear    | Poleturners Workshop  |    1     |          |        332        |     10    |
| Pike     | Poleturners Workshop  |    2     |          |        872        |     18    |
| Sword    | Blacksmiths Workshop  |          |    1     |       1090        |     30    |
| Mace     | Blacksmiths Workshop  |          |    1     |        910        |     30    |
| Armor    | Armourers Workshop    |          |    1     |        625        |     30    |

### Production Cycle Model

- One production cycle starts at the `Armoury` entry point and ends when the worker brings the finished weapon back to the `Armoury` entry point.
- Workers are simulated logically only. There is no need to animate worker movement in the engine.
- Workers carry only `1` resource unit per trip from stockpile to workshop.
- If a weapon needs `N` resource units, that means `N` separate stockpile-to-workshop deliveries inside the cycle.
- For most workshops the cycle starts:
  - `Armoury -> required stockpile -> Workshop`
- Fletcher special case:
  - default behavior: after a cycle ends at `Armoury`, the next cycle starts with `Armoury -> Fletchers Workshop -> Wood Stockpile`
  - optimized-fletcher setting: when enabled, Fletchers use the normal direct route and start with `Armoury -> Wood Stockpile`
- After all required resources are delivered to the workshop, the worker spends `Make Time (Ticks)` crafting the weapon.
- After crafting finishes, the worker goes `Workshop -> Armoury`, which ends the current cycle.
- The simulator can store completed cycle counts and totals instead of saving every tiny travel event.

### Bread Production

- Wheat and Flour are assigned to individual Goods Yard stockpiles.
- Wheat Farm work time is calibrated to 6950 ticks plus travel for 12 field-center/stockpile round trips. The loaded trip to stock uses `SB=1, SP=0` (16 ticks/cell); the empty return uses `SB=1, SP=1` (12 ticks/cell). The calibrated base time matches the measured three-farm reference layout at approximately 22 wheat/min.
- A farm produces 24 wheat per cycle at fear factor 0 and 36 at fear factor -5, interpolated linearly.
- A Wind Mill has three workers, but only one 312-tick wheat-to-flour processing operation can run at once.
- The mill worker loop is modeled as `Flour stockpile -> Wheat stockpile -> Mill -> Flour stockpile`.
- A Bakery cycle is `Granary -> Flour stockpile -> Bakery -> Granary` plus 1700 baking ticks.
- One flour produces 8 bread at fear factor 0 and 12 at fear factor -5, interpolated linearly.
- Bread throughput is the minimum of wheat supply, serialized mill capacity, and bakery capacity.
- `Buy Wheat` can fill a farm-wheat shortage through available Wind Mills; `Buy Flour` can fill the remaining Bakery input shortage. Farm wheat is used first, then bought wheat, then bought flour.
- Bought Wheat costs 23 gold/unit and bought Flour costs 32 gold/unit. Purchases cover Bakery demand only and are deducted from bread-economy total gold/min.

### Wall Object

- `Wall` is modeled as its own object, not as a building type.
- Wall placement uses two clicks:
  - first click sets start cell
  - second click sets end cell
- End cell must be horizontal or vertical from start (no diagonal walls).
- Wall occupies a 1-cell-thick line along all cells between start and end.
- UI settings include a **Remove All Walls** action.

### Worker Speed

- Unit movement speed function is:
  - `speed_cells_per_tick = 1 / (8 * (SB + 1))`
- Weapon workshops and Bakery use `SB = 2` (24 ticks per cell).
- Wheat Farm and Wind Mill use `SB = 1` (16 ticks per cell).

### Distance Objects

- Distances are directional objects between two buildings:
  - `(start_building_id, finish_building_id)`
  - reverse direction is a separate object
- Distance value is shortest cell-path length between start entry point and finish entry point.
- Neighbor cells include diagonal neighbors (8-direction movement).
- Occupied cells are blocked for path traversal.
- If start and finish entry points are the same cell, distance is `0`.
- Distance objects are stored in a map keyed by `(start_id, finish_id)`.

## Architecture

### Workspace

- root `Cargo.toml` defines workspace members
- each crate owns its own concerns

### `simulator` crate structure

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

### `gui` crate structure

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
