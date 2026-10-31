# Changelog

## [Unreleased]

### Added

- A system monitor for the terminal pane: cards for the CPU, memory, network and disks with the last
  minute of each as a graph, and a status line with the machine, its uptime and the update interval.
- Pause and resume the updates (Space, or the status line) and change how often the machine is looked
  at (0.5 to 10 seconds; `+` and `-`, or the status line).
- In a terminal that shows no views, or with `--once`, a summary of the machine is printed instead;
  `--json` prints the same as JSON, `--interval <seconds>` sets the update interval.
- A table of the processes under the cards: sort by any column, filter (words, `/regex/`, `user:`,
  `pid:`, `state:`, `cpu>`, `mem>`, `!` to negate), a tree, groups by name, folded branches that
  carry the totals of what they hide, the subtree of one process, marks, columns to show and hide,
  and an order that stays still under the pointer.
- The summary of `--once`, the pipe and `--json` lists the busiest processes.
- The details of a process in a panel beside the table (over it in a narrow pane): graphs of what it
  takes, the whole command line, working folder, parents and children (each one press away) and
  environment, with what looks like a secret hidden until asked.
- Signals (TERM, INT, HUP, KILL, STOP, CONT, USR1, USR2, to one process or to it and what is below
  it), a new priority, a shell in the process's folder, and copies of its command line, id and folder,
  from keys, buttons and the row's menu. A signal is asked for first, and goes to the process that
  was named (id and start): one that ended and whose id was used again is not signalled.
- The details of each card in place of the table: every processor with its own graph, its frequency
  and the temperatures; what memory is made of and swap; each network interface with its graph; the
  volumes and the reads and writes of the disks. A graph says what it held under the pointer and how
  long ago, and spans one, five or fifteen minutes.
- The listening ports: every TCP socket that waits for connections and every bound UDP socket, with
  the address and the process that holds it, filtered by port, name or user; a signal, a new priority
  or a shell for the row's process; Enter shows it in the table. `port:3000` in the process filter
  leaves the processes that listen there, and a process's details list its ports.
- The GPU: a card (where the machine has a GPU that says how busy it is) and its details: the use
  of each GPU over time, its memory, the parts of it that are busy and its temperature. Apple's
  accelerators, the DRM drivers of Linux and NVIDIA's driver are read; the disk's reads and writes
  are read on a Mac as well (the storage drivers), and the summary lists the GPUs.
- A palette (Cmd+K) that finds any command that can be done now and filters the table by what is
  typed, a list of every command with its keys (`?`), and htop's function keys as second keys.
- Settings (`~/.config/toppen/settings.json`, and a window for them): the update interval, how the
  table starts, whether names are shown, and the keys of every command, changed by pressing them.
  A key that the terminal, the table or another command has is refused with the reason; a file that
  cannot be read is left as it is.

