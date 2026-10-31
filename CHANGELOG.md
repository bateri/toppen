# Changelog

## [Unreleased]

### Added

- A system monitor for the terminal pane: cards for the CPU, memory, network and disks with the last
  minute of each as a graph, and a status line with the machine, its uptime and the update interval.
- Pause and resume the updates (Space, or the status line) and change how often the machine is looked
  at (0.5 to 10 seconds; `+` and `-`, or the status line).
- In a terminal that shows no views, or with `--once`, a summary of the machine is printed instead;
  `--json` prints the same as JSON, `--interval <seconds>` sets the update interval.
