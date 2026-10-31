# toppen

A system monitor for the terminal pane, in the way of `top` and `btop`: what the machine is doing,
as cards with graphs and a table of its processes, drawn by the terminal in its own font and colours. It runs on the machine it is
started on, so over plain `ssh` it shows the server's own CPU, memory, network and disks.

toppen is a [pencere](https://github.com/bateri/pencere) app: the program is Rust, the view is a
page of pencere's kit, and both are one file. A terminal that hosts pencere (bateri) shows the view
in the pane; any other terminal is given a summary of the machine, as `top -l 1` prints one.

![The overview](docs/overview.png)

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
