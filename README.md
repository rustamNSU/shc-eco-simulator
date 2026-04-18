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

Workshops and armoury currently use a 4x4 square footprint.

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

### Goods Buy/Sell Costs

| Goods | Sell | Buy |
| ----- | ---- | --- |
| Wood  |  1   |  4  |
| Iron  |  23  | 45  |

### Weapon Production and Sale

Empty cell means this resource is not needed.

Worker can carry only one resource unit from stockpile per trip.
If a weapon needs `2` wood or `2` iron, that means two separate stockpile-to-workshop paths/trips.

| Weapon   | Workshop              | Wood Req | Iron Req | Make Time (Ticks) | Sell Gold |
| -------- | --------------------- | -------- | -------- | ----------------- | --------- |
| Bow      | Fletchers Workshop    |    2     |          |        400        |     15    |
| Crossbow | Fletchers Workshop    |    3     |          |        565        |     30    |
| Spear    | Poleturners Workshop  |    1     |          |        300        |     10    |
| Pike     | Poleturners Workshop  |    2     |          |        600        |     18    |
| Sword    | Blacksmiths Workshop  |          |    1     |        600        |     30    |
| Mace     | Blacksmiths Workshop  |          |    1     |        600        |     30    |
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
- Workshops are the only current worker buildings.
- Workshop slowdown base coefficient is:
  - `SB = 2`

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
