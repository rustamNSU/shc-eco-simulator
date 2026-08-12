# SHC Eco Simulator

SHC Eco Simulator is a Windows desktop layout planner for Stronghold Crusader economy setups. Place buildings and walls on a grid, configure resources and production, and compare expected goods and gold per minute.

The simulator currently covers weapon workshops, the wheat-to-bread chain, stockpile routing, building costs, iron and stone production, and an optional population/tax/food/inn economy calculation.

## Download and run

The easiest way to use the app is to open the [GitHub Releases page](https://github.com/rustamNSU/shc-eco-simulator/releases/latest), download `SHCEcoSim.exe` from the latest release assets, and run it. No Rust installation is needed for the release executable.

Windows may show a SmartScreen warning for an unsigned community application. Confirm that the file came from this repository's Releases page before choosing to run it.

## Quick start

1. Select a building in the left column, then click a map cell to place it.
2. Place a Goods Yard. It creates four stockpiles; use the resource tools to mark the stockpiles used for Wood, Iron, Wheat, and Flour.
3. Add workshops and an Armoury for weapon production, or Farms, Wind Mills, Bakeries, and a Granary for bread production.
4. Add walls when you want to change a workshop or Bakery entry direction. The entry point is shown when **Show building entry points** is enabled.
5. Choose weapon types, Game Speed, Fear Factor, and which missing resources may be bought.
6. Press **Calculate Average**. After a map edit, the button becomes red and changes to **Recalculate** because route-dependent results are stale.
7. Hover over a building for its individual statistics. Use **Complete economy report** in the right column for totals and bottlenecks.

The canvas accepts negative coordinates and expands automatically, keeping at least 50 cells between placed objects and its boundaries.

## Interface

The main window has three columns:

- **Building and resource tools:** place buildings, walls, and stockpile resource designations.
- **Map and simulation:** edit the layout, zoom, remove objects, select workshop products, set speed/fear factor, choose purchasable inputs, and calculate results.
- **Settings & Economy:** show entry points and tooltips, enable optimized Fletcher routing, configure mines and population economy, and read/copy the complete report.

### Editing

- **Remove** selects a single-cell removal tool.
- **Remove All Walls** clears walls without removing buildings.
- **Remove All** clears the complete map.
- **Edit > Undo** or `Ctrl+Z` restores the previous map snapshot.
- **Edit > Redo** or `Ctrl+Y` reapplies an undone map snapshot.

Undo/redo covers building placement and removal, wall changes, stockpile resource assignments, entry-point changes caused by geometry, canvas expansion, and bulk removal. For example, **Remove All** can be undone in one step. A new map edit after undo clears the redo chain, and opening a project starts a new history.

Simulation settings are lightweight and update their calculations directly; they are not part of map undo/redo history.

### Reports and tooltips

**Show simulation tooltips** is enabled by default. Building tooltips report the relevant weapon, wheat, flour, bread, route, and gold rates.

The **Complete economy report** is a scrollable, read-only text field. Click inside it and use normal text selection or `Ctrl+A`, then `Ctrl+C`, to copy the report into a message, spreadsheet, or text file. Calculated paths and report text are regenerated rather than stored in project files.

## Save, open, and share projects

Use the **File** menu:

| Action | Shortcut | Behavior |
| --- | --- | --- |
| Open | `Ctrl+O` | Opens any compatible project JSON file |
| Open Recent | — | Opens the most recent project that still exists |
| Save | `Ctrl+S` | Saves to the current filename, or asks for one if the project is untitled |
| Save As | `Ctrl+Shift+S` | Saves a copy under another filename |

The default Windows project folder is:

```text
%LOCALAPPDATA%\SHCEcoSimulator\Projects
```

Projects are portable JSON files. They include canvas bounds, building IDs and coordinates, Goods Yard groups, stockpile resources, entry points, walls, weapon/resource settings, Game Speed, Fear Factor, mine counts, population, inns, tax, and food ratio. Opening a project reconstructs the layout and recalculates its building economy.

The recent-file list is local application state in `%LOCALAPPDATA%\SHCEcoSimulator\recent.json`; it is not part of a shared project.

To share an economy, send its `.json` file. The recipient can save it anywhere and open it with **File > Open**.

## Example economies

Ready-made layouts are available in [`examples/`](examples/). They include weapon-and-bread layouts, large bread economies, and smaller test layouts. Some examples use the current version 2 project format; the version 1 files demonstrate backward-compatible loading.

To use one without cloning the repository:

1. Open the JSON file on GitHub.
2. Choose **Download raw file** and keep the `.json` extension.
3. Start SHC Eco Simulator and select **File > Open**.

You can also download the repository ZIP and open an example directly from its extracted `examples` folder.

## Build from source

Install a current stable Rust toolchain, then run from the repository root:

```powershell
cargo run -p gui
```

Build an optimized Windows executable:

```powershell
cargo build --release -p gui
```

The locally built executable is `target\release\gui.exe`.

Run the validation suite with:

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace
```

## Developer documentation

- [Architecture](doc/architecture.md) — crate responsibilities, UI/backend data flow, history, persistence, and extension points.
- [Simulation model](doc/simulation_model.md) — placement, routing, production formulas, costs, and population economy rules.
- [Unit movement speed](doc/unit_movement_speed.md) — detailed tick and movement reference.
- [Example catalog](examples/README.md) — contents and purpose of each bundled project.
- [Rust development guidelines](skills/rust.md) — project coding and testing conventions.
