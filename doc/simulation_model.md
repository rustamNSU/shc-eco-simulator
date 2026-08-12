# Simulation model

This document records the rules currently implemented by SHC Eco Simulator. It is a technical reference; the main README focuses on using the application.

## Canvas and placement

- Coordinates are signed integer cells and may be negative.
- A building is positioned by its bottom-left cell `(x, y)`.
- Occupied cells cannot overlap another blocking building cell or wall.
- The canvas expands after placement when necessary to retain a 50-cell margin between every placed object and each boundary.
- Canvas bounds are persisted in version 2 project files and restored exactly.
- A Wheat Farm reserves a 9x9 area. Its 3x3 top-left cabin blocks paths; the remaining field cells are traversable.
- A Wind Mill is 3x3 with a fixed bottom-center entry.
- Workshops, Armouries, Bakeries, and Granaries use 4x4 footprints.
- A Goods Yard creates four independent 2x2 Stockpiles in a 5x5 template. Removing one member removes the full grouped Goods Yard.

## Entry points and walls

Buildings store an optional entry point. For an `n x n` square, the normal bottom candidate is `(x + floor(n/2), y - 1)`; a 2x2 Stockpile begins at `(x, y - 1)`.

If that candidate is blocked, side candidates are checked in orientation order, followed by corners. Walls can therefore force a workshop or Bakery to use a different side. The offset rotates with the building orientation, so the gate retains the same relative second-cell rule on every side. If no valid candidate is reachable, the entry point is absent.

Pathfinding uses eight-direction movement through unoccupied cells. Distances are directional `(start building, finish building)` records; the reverse direction is stored separately.

## Building costs

| Building | Wood | Gold |
| --- | ---: | ---: |
| Goods Yard / Stockpile | 0 | 0 |
| Armoury | 5 | 0 |
| Fletchers Workshop | 20 | 100 |
| Blacksmiths Workshop | 20 | 200 |
| Poleturners Workshop | 10 | 100 |
| Armourers Workshop | 20 | 100 |
| Wheat Farm | 15 | 0 |
| Wind Mill | 20 | 0 |
| Bakery | 10 | 0 |
| Granary | 5 | 0 |

The setup report always converts required purchased build wood to gold when showing the total. It also preserves separate workshop and food-economy cost breakdowns.

## Goods prices

| Good | Sell gold | Buy gold |
| --- | ---: | ---: |
| Wood | 1 | 4 |
| Iron | 23 | 45 |
| Wheat | 8 | 23 |
| Flour | 10 | 32 |
| Bread and other food | 4 | 8 |
| Stone | 7 | — |

Stone is produced and sold directly by the additional-resource calculator.

## Movement

General movement speed is:

```text
cells per tick = 1 / (8 × (SB + 1))
```

- Weapon workshops and Bakeries use `SB = 2`, or 24 ticks/cell.
- Wheat Farms and Wind Mills use `SB = 1`, normally 16 ticks/cell.
- A returning empty Wheat Farm worker uses 12 ticks/cell (`SB = 1`, `SP = 1`).

See [unit_movement_speed.md](unit_movement_speed.md) for the detailed movement reference.

## Weapon production

| Weapon | Workshop | Wood | Iron | Work ticks | Sell gold |
| --- | --- | ---: | ---: | ---: | ---: |
| Bow | Fletcher | 2 | 0 | 638 | 15 |
| Crossbow | Fletcher | 3 | 0 | 565 | 30 |
| Spear | Poleturner | 1 | 0 | 332 | 10 |
| Pike | Poleturner | 2 | 0 | 872 | 18 |
| Sword | Blacksmith | 0 | 1 | 1090 | 30 |
| Mace | Blacksmith | 0 | 1 | 910 | 30 |
| Armor | Armourer | 0 | 1 | 625 | 30 |

A worker carries one resource unit per trip. Normal cycles route from Armoury to the required Stockpile, then Workshop; multi-unit recipes repeat the Stockpile-to-Workshop delivery. Crafting time is added before the Workshop-to-Armoury return.

By default, a Fletcher's next cycle starts through the workshop before collecting wood. **Optimize Fletcher routing** changes it to the normal direct Armoury-to-Wood Stockpile route.

Workshop fear output uses a ten-cycle ring. At Fear Factor 0 the ring alternates one and two goods. Each step toward -5 changes one remaining single-output cycle to two.

### Iron supply

Iron mines produce only when the layout contains a Stockpile marked as Iron. Produced iron is used by workshops first. When **Buy Iron** is disabled, insufficient production scales iron workshop output down. When buying is enabled, only the deficit is bought at 45 gold. Any produced surplus is sold at 23 gold.

## Bread production

- A Wheat Farm cycle has 6,950 work ticks plus 12 loaded Farm-to-Wheat-Stock and empty return walks.
- A farm produces 24 wheat/cycle at Fear Factor 0 and 36 at -5, interpolated linearly.
- A Wind Mill has three workers, but wheat-to-flour processing is serialized at 312 ticks per unit plus travel.
- The Mill route is Flour Stockpile → Wheat Stockpile → Mill → Flour Stockpile.
- A Bakery route is Granary → Flour Stockpile → Bakery → Granary plus 1,700 work ticks.
- One flour produces 8 bread at Fear Factor 0 and 12 at -5, interpolated linearly.
- Actual bread throughput is limited by wheat supply, Mill capacity, or Bakery capacity.

When **Buy Wheat** is enabled, bought wheat fills available Mill/Bakery demand after farm production. When **Buy Flour** is enabled, bought flour fills remaining Bakery demand after produced flour. Input purchase costs are deducted from bread sale gold. Reports show produced and surplus wheat/flour, purchased inputs, bread production/sale gold, total gold, and the active bottleneck.

## Population and additional economy

Additional-resource inputs are always active; population-specific inputs apply only when **Count population economy** is checked.

### Food

Normal consumption is `0.6 food/person/min`. The discrete food ratios are:

| Ratio | Food multiplier | Popularity |
| --- | ---: | ---: |
| No food | 0.0x | -8 |
| Half | 0.5x | -4 |
| Normal | 1.0x | 0 |
| Extra | 1.5x | +4 |
| Double | 2.0x | +8 |

Bread eaten by the population cannot be sold. The complete-economy calculation deducts consumed bread from layout sale income and sells only the remainder.

### Tax

A game month is 800 ticks. Tax gold per minute is:

```text
population × coefficient × (game speed × 60 / 800)
```

| Popularity | Coefficient | Popularity | Coefficient |
| ---: | ---: | ---: | ---: |
| +7 | -1.0 | -6 | 1.0 |
| +5 | -0.8 | -8 | 1.2 |
| +3 | -0.6 | -12 | 1.4 |
| +1 | 0.0 | -16 | 1.6 |
| -2 | 0.6 | -20 | 1.8 |
| -4 | 0.8 | -24 | 2.0 |

### Inns

- Build cost: 20 wood + 100 gold.
- Operation: 11.2 gold/min and one worker per Inn.
- Capacity: 30 population per Inn.
- Popularity: +8 at full coverage, +6 above 75%, +4 above 50%, +2 above 25%, otherwise 0.

### Stone and iron

| Resource building | Build wood | Workers | Base production at FF 0 |
| --- | ---: | ---: | ---: |
| Stone Quarry + ox | 25 | 4 | 18.6 stone/min |
| Iron Mine | 20 | 2 | 2.63 iron/min |

Production rises linearly to 1.33x at Fear Factor -5. Stone is valued at 7 gold/unit. Iron follows the workshop deficit/surplus rules above.

Total popularity is Tax + Food + Inn + Fear Factor. The UI treats zero or higher as acceptable and negative totals as a warning. Total gold combines workshop income, bread remaining after food, tax, stone sales, iron savings/surplus sales or purchases, and Inn beer expense.
