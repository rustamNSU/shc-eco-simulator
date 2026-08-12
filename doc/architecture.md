# Architecture

SHC Eco Simulator is a Rust workspace with a deterministic simulation crate and a Slint desktop GUI crate. The simulation layer does not depend on the UI or filesystem, which keeps project loading, pathfinding, and economy calculations testable.

## Workspace

```text
shc-eco-simulator/
├── simulator/         Domain model and calculations
├── gui/               Windows desktop application
├── examples/          Portable project JSON examples
├── doc/               Technical and domain documentation
├── skills/            Repository development guidelines
└── Cargo.toml          Cargo workspace definition
```

## `simulator` crate

`simulator` owns all game/layout state and calculations. Its public API is re-exported from `simulator/src/lib.rs`.

### Buildings

`simulator/src/buildings/` contains:

- `types.rs`: building types, display names, build costs, and worker movement speed.
- `placement.rs`: placed building identity, coordinates, resource assignment, entry point, footprint, and runtime components.
- `footprint.rs`: occupied/traversable cell templates, including the Wheat Farm cabin/field distinction.
- `entry_point.rs`: the stored entry-point coordinate type.
- `factory.rs`: creation of normal buildings and grouped Goods Yard stockpiles.
- `stockpile_resource.rs`: Wood, Iron, Wheat, and Flour designations.

### Map and walls

`simulator/src/map/` owns signed canvas bounds, cell occupancy, collision validation, and map errors. Bounds are signed because layouts may extend into negative coordinates.

`simulator/src/walls/` owns horizontal and vertical wall segments. Walls participate in occupancy and pathfinding and can reorient building entries.

### Simulation

`simulator/src/simulation/` is split by responsibility:

- `simulator.rs`: high-level layout operations, automatic canvas expansion, removal, distance rebuilding, and production-cycle entry points.
- `entry_logic.rs`: entry-point candidates and wall-controlled orientation.
- `pathfinding.rs` and `distance.rs`: directional shortest-path results between building entries.
- `production_cycle.rs`: workshop recipes, travel/crafting ticks, resource requirements, and weapon output.
- `bread.rs`: Wheat Farm, Wind Mill, Bakery, and Granary throughput and bottlenecks.
- `population_economy.rs`: population, workers, food use, tax, inns, mines, resource deficits/surpluses, and total gold.
- `game_time.rs` and `worker_distance.rs`: time conversion and worker-route details.

### Project format

`simulator/src/project.rs` defines the serializable `ProjectFile` schema. Project files contain source state only, not derived routes or reports. Loading validates IDs and geometry, rebuilds the `Simulator`, normalizes calculator settings, and accepts both legacy version 1 and current version 2 files.

Version 2 stores explicit `MapBounds`; version 1 stores a square `map_size`. This difference allows current projects to preserve expanded and negative-coordinate canvases.

## `gui` crate

The GUI uses Slint for rendering and Rust for behavior.

- `gui/ui/main.slint`: declarative three-column layout, menus, controls, canvas, tooltips, and report fields.
- `gui/src/app.rs`: callback wiring, UI refresh, summary/report formatting, project open/save coordination, and settings updates.
- `gui/src/editor_state.rs`: UI-facing state and selection/settings management.
- `gui/src/backend.rs`: background simulation command loop and map history.
- `gui/src/project_files.rs`: JSON file dialogs, default app-data folder, and recent-file tracking.
- `gui/src/visuals.rs`: conversion of domain objects into render models.

## Data flow

```text
Slint input
   │
   ▼
app.rs callback ───────────────► lightweight setting update
   │                                  │
   │ geometry command                 └──► refresh reports/view
   ▼
backend worker thread
   │
   ├── mutate Simulator
   ├── record map history
   └── return BackendUpdate
             │
             ▼
       EditorState + Slint models
```

Geometry operations run through the backend thread so large maps do not block event handling. Building/wall edits defer route recalculation and mark results stale. The expensive distance rebuild happens when the user presses **Recalculate**, not after every placement. Lightweight population and resource settings can refresh immediately.

## Undo and redo

`MapHistory` stores complete `Simulator` snapshots in undo and redo stacks. A successful geometry/resource edit records the previous snapshot and clears redo. Undo swaps the active simulator with the last snapshot; redo performs the reverse.

This intentionally favors straightforward, reliable restoration over a complicated inverse-command system. It restores grouped Goods Yards, stockpile assignments, entry points, walls, and canvas bounds together. Loading a project clears history because the loaded file becomes the new session baseline.

## Persistence flow

On save, `app.rs` captures:

1. the current `Simulator` layout;
2. `SimulationSettings`;
3. `PopulationEconomySettings`.

`project_files.rs` serializes the capture as pretty JSON. On open, JSON is decoded and validated in `simulator`, then the backend rebuilds worker distances and workshop-cycle results before updating the UI. Derived results remain out of the file so format compatibility does not depend on report implementations.

## Adding a feature

Keep domain rules in `simulator` and UI presentation in `gui`:

1. Add or change domain data and deterministic calculations in the narrowest simulator module.
2. Export only the types/functions the GUI needs.
3. Add focused simulator unit tests for formulas, placement, paths, and serialization behavior.
4. Add a backend command only when the operation mutates map state or is expensive.
5. Wire the control and rendering in `app.rs`/`main.slint`.
6. Update `doc/simulation_model.md` when a game constant or formula changes.
7. Run formatting, Clippy, and all workspace tests.

Repository-specific Rust guidance is in [`skills/rust.md`](../skills/rust.md).
