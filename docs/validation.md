# Validation

Pintle GPUI Kit uses Rust 1.99.0 and GPUI Kit 0.8.1. The local validation platform
is macOS. CI also defines Linux and Windows coverage; local results do not imply
those platform jobs have run or passed.

## Required checks

```sh
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --no-deps --locked
```

Check the facade independently with `--no-default-features` and each of: no
features, `theme`, `components`, `shell`, `assets`, `shell,assets`, `terminal`,
`terminal,shell`; also check `--all-features`.

The workspace includes the Gallery and starter. Run the starter directly with
`cargo run --locked -p pintle-gpui-starter`; its tests are included in the workspace
suite. Independent applications must also validate their own Git pin and lockfile.

## Coverage

The current suite contains 99 passing tests and two compiled doctests. Two manual
CPU workloads are ignored in normal test runs. Coverage includes:

- Workspace preview/permanent tabs, retained views, docking, splits and resizing.
- Rounded frames, clipping, sidebar visibility, configurable content and focus
  restoration, command filtering and stable selection IDs.
- Navigation search/expansion restoration, initial expansion, custom row heights,
  fonts and caller-owned trailing decoration.
- Resource-table/header composition using an independent delegate and caller
  slots, including 36px table rows, optional floating actions, caller callbacks
  and dismissal without resizing the table viewport or moving its rows.
- Editable-table validation, atomic edits/paste, stable selection keys,
  keyboard/pointer interaction, menus, shared split data and retained scroll state.
- Optional terminal parser, native input, bounded history, resizing, process
  lifecycle and real PTY regressions.
- Default and neutral themes, overrides and retained state across mode changes.

## Visual and platform limits

Native Gallery inspections cover light/dark at 1440 × 900 and 1024 × 680,
including title/activity/status bars, typography, folder tabs and rounded frames.
Rendered tests verify geometry, keyboard interactions and resizing. These are not
automated GPU pixel-diff tests or input-to-display latency measurements.

Lightship is an independent consumer of the public facade. It owns offline
Kubernetes fixtures and resource behavior; the kit owns shared UI composition.
Lightship validates its own navigation, filtering, sorting, selection, panels,
retained tabs and splits against its pinned kit commit.

The upstream `block 0.1.6` future-compatibility warning remains. It does not fail
the pinned toolchain's checks. Windows/Linux coverage stays in CI and must be
reported according to actual job results.

## Manual performance workloads

`editable_scrolling_cpu_reference` compares plain upstream and editable-table
scrolling under the same rendered workload. `workbench_cpu_workloads` exercises
palette, table, tabs, divider and Output behavior. These tests are opt-in:

```sh
cargo test --locked -p pintle-gpui-gallery editable_scrolling_cpu_reference -- --ignored --nocapture
cargo test --locked -p pintle-gpui-gallery workbench_cpu_workloads -- --ignored --nocapture
```

See [performance measurements](PERFORMANCE.md) for the recorded methodology,
results and limitations. CPU workload timings are not native frame-rate claims.
