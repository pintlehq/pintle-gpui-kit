# Gallery performance — 2026-11-03

The runtime changes keep the existing Cargo profiles, GPUI Kit 0.8.1, public
Foundation APIs and document retention/validation behavior. The command palette
now reserves a 360 px content height while its results change.

## Changes

- Workspace notifications ignore repeated activation/selection of the same
  group/document while still restoring document focus. Selection and activity
  changes update the inspector without rebuilding Workbench chrome. Layout
  comparisons run only when the layout snapshot changes.
- Table keys are shared snapshots with row/column position maps. Pointer and
  keyboard movement no longer copy all keys or scan them for an active position.
  Reconciliation updates the maps after data/filter/sort changes. Unchanged cell
  selections and repeated pointer movement inside one cell avoid notifications.
- Gallery stores cache Unicode lowercase cell text and numeric sort keys. An
  atomic commit updates only the changed keys and one source revision. Each
  retained view rebuilds its projection once per source revision/query/sort.
- Edge autoscroll follows frame callbacks and elapsed time at 562.5 logical
  pixels/second (the previous 18 px/32 ms rate), with a 50 ms stall cap. It has
  no middle-of-table polling timer and stops at scroll limits. Generation checks
  invalidate callbacks on release, Escape and data reconciliation; callbacks
  retain weak entities.
- Command entries and display labels are prepared with the catalog. The fixed
  palette keeps the input/footer stable for many, few and zero results.
- Output uses a uniform virtual list of display lines with horizontal scrolling.
  Multiline log entries retain their line breaks; their display-line cache is
  rebuilt lazily when Output is rendered after changes. It retains the
  newest-first order, 500-entry cap and pixel scroll offset. Hidden Output does
  not notify the entire Workbench for each new activity message.

## Measurement

The baseline is a copy of the working tree before the runtime changes, including
the uncommitted Command palette. Both versions use the same manual workload
source. Test executables were copied outside Cargo's target directory before
measurement, so rebuilding another checkout cannot replace a running baseline.

Each profile/version runs three times. Each workload discards five warm-up
operations and records 30 operations, reporting the middle sample and nearest-rank
p95. The table below takes the median of the three reported medians/p95 values;
individual repeats are retained in `performance/2026-11-03.json`. Initial release
results had apparent 5–10% median regressions, so release was repeated as three
before/after pairs with frozen binaries and no build/test commands during
timing. The release table uses those paired runs; the initial release records
are also retained. Dev uses its original three repeats. A final one-pass release
check after adding multiline-log preservation is retained as `final_check`; it
overlapped workspace validation and is excluded from the comparison table.

Every window has the six 1,000-row resources, four Foundation pages, ten retained
tabs and a right split. Both 1024 × 680 and 1440 × 900 are exercised in light mode.
The five workloads are:

- `palette`: open, type `public`, move down, clear, close, with separate UI turns.
- `table`: scroll seven rows forward and change the selected name cell, then draw.
- `tabs`: activate the next retained resource/Foundation tab and draw.
- `divider`: a native-event sidebar drag alternating 12 px in either direction.
- `output500`: append an activity to a full visible Output panel and draw.

These are **CPU milliseconds per whole operation**, including the test helpers'
forced draws and deferred event delivery. A palette operation includes several
draws. They are not milliseconds per native frame, GPU presentation time,
display FPS or measured input-to-display latency. Dev and release results are
compared only within their own unchanged profiles. The machine was not isolated
from other desktop activity; repeat variation and p95 spikes are retained rather
than treated as a statistically established speedup.

## Results

| Profile | Window | Workload | Median before → after (ms) | p95 before → after (ms) | Median change |
| --- | --- | --- | ---: | ---: | ---: |
| dev | 1024x680 | palette | 969.99 → 995.21 | 1145.37 → 1071.88 | +2.6% |
| dev | 1024x680 | table | 146.46 → 142.86 | 167.45 → 147.20 | -2.5% |
| dev | 1024x680 | tabs | 101.78 → 102.40 | 120.79 → 106.69 | +0.6% |
| dev | 1024x680 | divider | 517.55 → 499.40 | 555.63 → 507.76 | -3.5% |
| dev | 1024x680 | output500 | 213.71 → 103.78 | 234.29 → 108.84 | -51.4% |
| dev | 1440x900 | palette | 1404.72 → 1367.46 | 1630.72 → 1481.53 | -2.7% |
| dev | 1440x900 | table | 208.66 → 196.79 | 216.92 → 208.21 | -5.7% |
| dev | 1440x900 | tabs | 140.28 → 141.94 | 149.44 → 148.80 | +1.2% |
| dev | 1440x900 | divider | 687.89 → 693.81 | 795.04 → 729.05 | +0.9% |
| dev | 1440x900 | output500 | 276.44 → 148.15 | 289.78 → 168.72 | -46.4% |
| release | 1024x680 | palette | 116.14 → 116.95 | 126.69 → 125.89 | +0.7% |
| release | 1024x680 | table | 16.65 → 16.19 | 20.20 → 18.50 | -2.8% |
| release | 1024x680 | tabs | 11.61 → 11.62 | 14.68 → 13.43 | +0.1% |
| release | 1024x680 | divider | 59.05 → 58.65 | 64.28 → 65.32 | -0.7% |
| release | 1024x680 | output500 | 26.59 → 12.35 | 30.47 → 14.12 | -53.6% |
| release | 1440x900 | palette | 172.90 → 169.08 | 216.47 → 182.91 | -2.2% |
| release | 1440x900 | table | 24.57 → 24.05 | 28.50 → 27.78 | -2.1% |
| release | 1440x900 | tabs | 16.87 → 17.07 | 20.27 → 19.72 | +1.2% |
| release | 1440x900 | divider | 86.57 → 86.21 | 95.05 → 92.75 | -0.4% |
| release | 1440x900 | output500 | 34.84 → 18.38 | 38.65 → 20.96 | -47.2% |

Output is the clear CPU improvement: release medians fall 47–54% in the paired
runs. Other paired release medians stay within roughly 3% of baseline; these
numbers do not establish a broad table/tab/resize speedup. Their notification,
allocation and idle-work reductions are covered independently by deterministic
tests. The fixed palette addresses movement caused by changing result counts.

## Verification

Deterministic regressions cover notification isolation, stable palette geometry,
virtualized Output/scroll retention, one projection rebuild per shared edit,
Unicode cache updates, and no autoscroll work in the middle or after cancellation.
Existing tests retain sorted/filtered stable keys, atomic rejection, shared
split edits, keyboard/pointer dropdowns, docking and rounded frame coverage.

Final verification passed formatting, workspace check, Clippy with warnings
denied, all 58 workspace tests and two compiled doctests. The two manual CPU
workloads remain ignored in normal tests. Bare/theme/components/shell/assets/
shell+assets/all facade checks and docs with warnings denied also passed.

A separate release Gallery was inspected natively in light and dark at the
initial 1440 × 900 size: typing focused the palette immediately, filtered Enter
activation opened the correct resource, empty results kept the same 360 px
palette geometry, wheel scrolling advanced the table, and Output displayed
newest-first activity. A CUA sidebar-drag attempt left the geometry unchanged,
so this run does **not** claim native resize verification. Rendered pointer
regressions cover both sizes, divider movement, range dragging, edge scrolling,
release/cancellation and the final scroll limit. No native-FPS or GPU benchmark
is claimed.


## Reproduce

```sh
cargo test --locked -p pintle-gpui-gallery workbench_cpu_workloads -- --ignored --nocapture
cargo test --release --locked -p pintle-gpui-gallery workbench_cpu_workloads -- --ignored --nocapture
```

Run the workloads alone, three times per profile. Compare separate frozen
baseline/candidate executables, alternating versions for release comparisons;
do not reuse a mutable Cargo test executable across checkouts. The existing
`editable_scrolling_cpu_reference` workload remains available for the isolated
plain-Kit/editable-table comparison described in `validation.md`.
