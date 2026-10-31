# toppen

A system monitor for the terminal pane, in the way of `top` and `btop`: what the machine is doing,
as cards with graphs and a table of its processes, drawn by the terminal in its own font and colours. It runs on the machine it is
started on, so over plain `ssh` it shows the server's own CPU, memory, network and disks.

toppen is a [pencere](https://github.com/bateri/pencere) app: the program is Rust, the view is a
page of pencere's kit, and both are one file. A terminal that hosts pencere (bateri) shows the view
in the pane; any other terminal is given a summary of the machine, as `top -l 1` prints one.

![The overview](docs/overview.png)

![The details of a process](docs/details.png)

![The processor at length](docs/card-cpu.png)

## Use

```
toppen                    # the monitor, in a terminal that shows pencere views
toppen --once             # print a summary of the machine and exit
toppen --json             # the same summary, as one line of JSON
toppen --interval 2       # look at the machine every 2 seconds (0.5 to 10)
```

- **Space** pauses the updates and resumes them; the status line says when they are paused.
- **+** and **-** change how often the machine is looked at, from 0.5 to 10 seconds.
- The table lists the processes, the busiest first. Press a column's name to sort by it (again to
  reverse), drag the edge of a name to size the column, right-press a name to show or hide columns.
  **<** and **>** step the sort column, **r** reverses it.
- **/** filters: words match the name and the command line, `/regex/`, `!` for what does not match,
  `user:root`, `pid:42`, `state:R`, `cpu>50`, `mem>500M`. **Esc** clears it.
- **t** shows the processes as a tree, **g** groups the ones of one name, **f** folds or opens a
  branch (**[** and **]** every branch), **o** narrows the table to one process and what is below it,
  **m** marks a process.
- While the pointer is over the table (and for three seconds after a key) the rows keep their places,
  however the numbers change; "Order held" says how many would have moved, and pressing it lets go.
- **Enter** (or a double click) shows a process in a panel beside the table: what it takes, as graphs
  of the last minutes, its whole command line, where it works, who started it and what it started
  (press one to go to it), and its environment (what looks like a secret stays hidden until asked).
  **i** shows or hides it, **Esc** puts it away.
- What can be done to the process under the cursor: **k** sends a signal (a menu of TERM, INT, HUP,
  KILL, STOP, CONT, USR1 and USR2; toppen asks first, showing the whole command line, and offers to
  send it to what is below it too), **n** changes its priority, **s** opens a shell in its folder,
  **c** copies its command line, and **.** (or the right button) opens the menu of the row, which
  has these and more. A process of another user is shown, and left alone unless toppen runs as root.
  A signal names the process by its id and the second it started in: if it ended meanwhile and its
  id was used again, nothing is sent.
- Press a card (or **1** CPU, **2** memory, **3** network, **4** disk) for its details in place of the
  table; **Esc** goes back. CPU: every processor with its own graph, frequencies and temperatures;
  memory: what it is made of (app, wired, compressed, cached files, free) and swap; network: each
  interface with its own graph (the arrows choose); disk: every volume and the reads and writes. Rest
  the pointer on a graph to read what it held and how long ago; **w** (or the buttons) changes how far
  back the graphs look: 1, 5 or 15 minutes.
- Everything the keys do can be pressed with the mouse.

The program looks at the machine only while its view is open and the updates are not paused.

## Build

```
make fake-host   # once: the stand-in terminal the tests run toppen in (fetches pencere)
make check       # format, lints, the program's tests, the page's tests
make build       # target/release/toppen
make dev         # run it from its files in a pane that hosts pencere
```

`docs/calls.md` is the contract between the page and the program.

## License

GPL-3.0-or-later.
