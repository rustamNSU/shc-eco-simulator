# Unit Movement Speed Formula

## Formula

For one unit to move by **1 cell**:

\[
N_t = 8 \times \frac{1 + SpeedUp + SB}{1 + SpeedUp}
\]

Where:
- `Nt` = game ticks per 1 cell
- `SpeedUp` = speed-up coefficient (`unit + 0x916`, WORD)
- `SB` = actual slowdown (`unit + 0x9A2`, WORD)
- Ford surface (`brod`, shallow water crossing) adds `+2` to `SB` while the unit is moving on it.
- Oil marsh surface adds `+6` to `SB` while the unit is moving on it.

For distance `D` cells at game speed `GS`:

\[
T_{seconds} = \frac{D \times N_t}{GS}
\]

---

## Unit Coefficients Table

Offsets:
- `SpeedUp` at `0x916` (WORD)
- `SB` at `0x9A2` (WORD)

| Unit                       | ID | SB (slowdown) | SpeedUp |
|----------------------------|---:|--------------:|--------:|
| `UNIT_PEASANT`             |  1 |             1 |       0 |
| `UNIT_WOODCUTTER`          |  3 |             2 |       0 |
| `UNIT_FLETCHER`            |  4 |             2 |       0 |
| `UNIT_QUARRY_MASON`        |  7 |             1 |       0 |
| `UNIT_QUARRY_GRUNT`        |  8 |             1 |       0 |
| `UNIT_QUARRY_OX`           |  9 |             1 |       0 |
| `UNIT_PITCHMAN`            | 10 |             1 |       0 |
| `UNIT_FARMER_WHEAT`        | 11 |             1 |       0 |
| `UNIT_FARMER_HOPS`         | 12 |             1 |       0 |
| `UNIT_FARMER_APPLE`        | 13 |             1 |       0 |
| `UNIT_FARMER_CATTLE`       | 14 |             1 |       0 |
| `UNIT_MILLER`              | 15 |             1 |       1 |
| `UNIT_BAKER`               | 16 |             2 |       0 |
| `UNIT_BREWER`              | 17 |             1 |       0 |
| `UNIT_POLETURNER`          | 18 |             2 |       0 |
| `UNIT_BLACKSMITH`          | 19 |             2 |       0 |
| `UNIT_ARMOURER`            | 20 |             2 |       0 |
| `UNIT_ARCHER`              | 22 |             1 |       1 |
| `UNIT_PIKEMAN`             | 25 |             2 |       0 |
| `UNIT_MACEMAN`             | 26 |             1 |       1 |
| `UNIT_SWORDSMAN`           | 27 |             4 |       0 |
| `UNIT_KNIGHT`              | 28 |             1 |       2 |
| `UNIT_PRIEST`              | 33 |             2 |       0 |
| `UNIT_INNKEEPER`           | 36 |             3 |       0 |
| `UNIT_MONK`                | 37 |             2 |       0 |
| `UNIT_LORD`                | 55 |             2 |       0 |
| `UNIT_PORTABLE_SHIELD`     | 60 |             1 |       1 |
| `UNIT_ARAB_BOW`            | 70 |             1 |       1 |
| `UNIT_ARAB_SLAVE`          | 71 |             1 |       1 |
| `UNIT_ARAB_SLINGER`        | 72 |             1 |       1 |
| `UNIT_ARAB_HORSEMAN`       | 74 |             1 |       2 |
| `UNIT_ARAB_SWORDSMAN`      | 75 |             3 |       0 |
| `UNIT_BEDOUIN_CAMEL_LANCER`| 78 |             1 |       2 |
| `UNIT_BEDOUIN_EUNUCH`      | 80 |             4 |       0 |
| `UNIT_BEDOUIN_SKIRMISHER`  | 82 |             1 |       1 |
| `UNIT_BEDOUIN_HEAVY_CAMEL` | 83 |             1 |       2 |

Note:
- `UNIT_MACEMAN` uses `SB=1`, `SpeedUp=1` in normal gameplay.
- In map editor it can appear as `SpeedUp=0` (known bug/inconsistency).

---

## Quick Tick Reference (1 cell)

| SpeedUp | SB | Nt (ticks) |
|---|---:|---:|
| 0 | 1 | 16 |
| 0 | 2 | 24 |
| 0 | 3 | 32 |
| 0 | 4 | 40 |
| 1 | 1 | 12 |
| 2 | 1 | 10.6667 |

---

## Army Units (Requested Set)

| Unit                        | ID | SB (slowdown) | SpeedUp | HP    | Gold Cost | Time 100 cells @ GS 50 (s) |
|-----------------------------|---:|--------------:|--------:|------:|----------:|---------------------------:|
| `UNIT_ARAB_ASSASIN`         | 73 |             1 |       0 | 15000 |        60 |                      32.00 |
| `UNIT_ARAB_HORSEMAN`        | 74 |             1 |       2 | 10000 |        80 |                      21.33 |
| `UNIT_PIKEMAN`              | 25 |             2 |       0 | 50000 |        20 |                      48.00 |
| `UNIT_MACEMAN`              | 26 |             1 |       1 | 15000 |        20 |                      24.00 |
| `UNIT_KNIGHT`               | 28 |             1 |       2 | 20000 |        40 |                      21.33 |
| `UNIT_ARAB_SWORDSMAN`       | 75 |             3 |       0 | 20000 |        80 |                      64.00 |
| `UNIT_BEDOUIN_CAMEL_LANCER` | 78 |             1 |       2 | 12000 |        50 |                      21.33 |
| `UNIT_BEDOUIN_SKIRMISHER`   | 82 |             1 |       1 | 15000 |        25 |                      24.00 |
| `UNIT_BEDOUIN_SAPPER`       | 84 |             1 |       0 | 10000 |        50 |                      32.00 |

Damage cross-table (`unit.json`, row = attacker, column = defender):

| Attacker \\ Defender         | `ASS` | `HA` | `PIKE` | `MACE` | `KNG` | `AR_SWD` | `CAM` | `SKIR` | `SAP` |
|------------------------------|------:|-----:|-------:|-------:|------:|---------:|------:|-------:|------:|
| `UNIT_ARAB_ASSASIN`          |    80 |   80 |     80 |     80 |    80 |       80 |    80 |     80 |    80 |
| `UNIT_ARAB_HORSEMAN`         |    20 |   20 |     20 |     20 |    20 |       20 |    20 |     20 |    20 |
| `UNIT_PIKEMAN`               |    20 |   20 |     20 |     20 |    20 |       20 |    20 |     20 |    20 |
| `UNIT_MACEMAN`               |    75 |   75 |     75 |     75 |    25 |       75 |    75 |     75 |    75 |
| `UNIT_KNIGHT`                |    80 |   80 |     80 |     80 |    50 |       80 |    80 |     80 |    80 |
| `UNIT_ARAB_SWORDSMAN`        |   100 |  100 |    100 |    100 |    50 |      100 |   100 |    100 |   100 |
| `UNIT_BEDOUIN_CAMEL_LANCER`  |    80 |   80 |     40 |     70 |    25 |       30 |    80 |     80 |    80 |
| `UNIT_BEDOUIN_SKIRMISHER`    |    15 |   15 |     15 |     15 |    15 |       15 |    15 |     15 |    15 |
| `UNIT_BEDOUIN_SAPPER`        |    25 |   25 |     25 |     25 |    25 |       25 |    25 |     25 |    25 |

Legend:
- `ASS` = `UNIT_ARAB_ASSASIN`
- `HA` = `UNIT_ARAB_HORSEMAN`
- `PIKE` = `UNIT_PIKEMAN`
- `MACE` = `UNIT_MACEMAN`
- `KNG` = `UNIT_KNIGHT`
- `AR_SWD` = `UNIT_ARAB_SWORDSMAN`
- `CAM` = `UNIT_BEDOUIN_CAMEL_LANCER`
- `SKIR` = `UNIT_BEDOUIN_SKIRMISHER`
- `SAP` = `UNIT_BEDOUIN_SAPPER`

