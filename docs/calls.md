# The page and the program

The page (`src/view/`) and the program (`src/program/`) talk only through what is written here:
calls the page makes, and topics it listens to. A change to one starts in this file. The code that
answers is `src/program/api/`; the page's own side is `src/view/parts/` (`overview.lua`, `table.lua`,
`filter.lua`, `status.lua`, `detail.lua`, `actions.lua`).

**Units.** Bytes are bytes (1024 to the step when a person reads them), a share is a number from 0 to
100, time is seconds, a rate is bytes a second, a frequency is MHz. Members are `camelCase`.

**Errors.** A call that cannot be done answers with an error: `bad-params` (the params are not what
the call takes: a member it does not know, a value out of range; the words say which), `not-found`
(no such call), `internal` (the program failed), `gone` (the process asked about has ended, or is not
the one that was named: its start does not hold), `unavailable` (the thing asked for cannot be done
here, as the shell in a terminal that cannot lend its pane). The words of an error are for the person.

## Calls

### `boot`

What the page needs before it draws, once. Params: none.

```json
{
  "version": "0.1.0",
  "person": { "name": "lantern", "root": false },
  "machine": {
    "host": "lantern", "os": "macOS 26.4.1", "kernel": "25.4.0", "arch": "arm64",
    "cpu": "Apple M1 Pro", "threads": 10, "cores": 10,
    "memory": 34359738368, "booted": 1761600000
  },
  "features": { "pressure": true, "diskIo": true, "gpu": true },
  "sampling": { "interval": 1.0, "paused": false, "min": 0.5, "max": 10.0 }
}
```

`person` is who is looking: the processes of that `name` (all of them, as `root`) are the ones the
page lets be signalled and reniced; the system has the last word (`denied`).

`features` says what this machine can measure: a card whose reading is not there is drawn without
it, not with a zero.

- `pressure`: the system's own measure of how pressed memory is (macOS's memory pressure, Linux's PSI).
- `diskIo`: reads and writes of the disks, in bytes a second.
- `gpu`: at least one graphics processor that says how busy it is (Apple's accelerators, the DRM
  drivers of Linux, NVIDIA's driver). Without it there is no GPU card and no GPU in the samples.

### `sampling`

How often the machine is looked at, and whether it is. Params: `interval` (seconds, 0.5 to 10) and
`paused` (boolean), both optional. Answers what is now so: `{ "interval", "paused", "min", "max" }`.
While paused the program looks at nothing.

### `card`

Which card's details the person has open, so that the program reads what costs more (the
temperature sensors) only then. Params: `card`, one of `cpu`, `memory`, `network`, `disk`, `gpu`, or
`null` for none. Answers `{ "card" }`. The sensors are in the samples that follow, within an interval.

### `view`

How the process table is ordered, narrowed and folded, and which rows are in view. Every member is
optional; what is left out stays as it was, and the table never changes by anything but this call.
Answers the view as it now is. The topic `processes` tells the rows again, in the new shape.

```json
{ "sort": "cpu", "descending": true, "mode": "tree", "filter": "chrome user:lantern",
  "hold": true, "root": { "pid": 412, "start": 1761860000 },
  "window": { "first": 120, "count": 40 },
  "selected": { "pid": 4120, "start": 1761865000 },
  "toggle": { "pid": 412, "start": 1761860000 }, "fold": "all" }
```

- `sort` is one of `pid name user cpu memory virt threads state nice time read write command`;
  equal values are ordered by pid, in both directions. `mode` is `flat`, `tree` or `group`.
- `filter` is the terms the person typed after `/`, separated by spaces, all of which must hold:
  a word (in the name or the command, whatever the case; `"two words"` for a phrase), `/regex/`
  (whatever the case), `!term` (what does not hold), `user:root,www`, `pid:1,42`, `port:8080`,
  `state:RD`, `cpu>50`, `cpu<5` (percent of one processor), `mem>500M`, `mem<2G` (resident; a bare
  number is megabytes). A filter that is not one leaves the last good filter in force and comes back
  as `error` of the view. At most 400 characters.
- `hold` keeps the rows in the places they were last told in while it is `true`, however the numbers
  change: new processes go last, and a process that ended stays one sample as an `exited` row (flat
  list only). Turning it off, or any change of `sort`, `descending`, `mode`, `filter` or `root`,
  gives the order of the moment.
- `root` narrows the table to that process and what is below it; `null` leaves. The table goes whole
  by itself when the process ends.
- `window` is the first row in view and how many are in view (at most 200 are told).
- `selected` is a row the page keeps its cursor on (`{ "group": "name" }` for a group); the topic says
  where it is. `null` selects nothing.
- `toggle` folds an open row or opens a folded one, once: a branch of the tree, or a group.
  `fold` is `all` or `none`, once: fold every branch (every group) or open every one.

A process is named by its `pid` and `start` (seconds since the epoch): a pid is used again after its
process ends.

### `detail`

What a measurement does not say of one process. Params: `pid` and `start`, both required. Answers:

```json
{ "pid": 4120, "start": 1761865000, "ppid": 412, "name": "Google Chrome", "user": "lantern",
  "state": "S", "nice": 0, "threads": 41, "cpu": 12.5, "mem": 838860800, "virt": 4398046511104,
  "time": 1032.4, "read": 0, "write": 4096,
  "argv": ["/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", "--type=renderer"],
  "exe": "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome", "cwd": "/",
  "env": [{ "name": "API_TOKEN", "value": "tok-8f2a", "secret": true }],
  "fds": 112,
  "parents": [{ "pid": 412, "start": 1761860000, "name": "Google Chrome", "cpu": 3.1 }],
  "children": [{ "pid": 4188, "start": 1761865100, "name": "helper", "cpu": 0.2 }],
  "childrenTotal": 1, "may": true,
  "ports": [{ "port": 3000, "proto": "tcp", "addr": "127.0.0.1" }] }
```

- `argv` is the command line a word each; `exe`, `cwd`, `env` and `fds` (open files) are what the
  system tells the person to see (`null`, or an empty `env`, where it says nothing). `secret` is true
  of a variable whose name has `TOKEN`, `SECRET`, `PASSWORD`, `PASSWD`, `KEY`, `CREDENTIAL`, `AUTH` or
  `COOKIE` in it; the page hides its value until asked.
- `parents` go from the nearest to the oldest (at most 24); `children` are the busiest 40 of those
  below, `childrenTotal` all of them.
- `may` is whether the person may signal and renice it (it is theirs, or they are root).
- `ports` are the sockets it listens on (see the topic `ports`).
- A process that is not in the last sample, or whose `start` does not hold, is `gone`.

### `signal`

Sends a signal to a process, once. Params: `pid`, `start` and `signal` (one of `TERM INT HUP KILL
STOP CONT USR1 USR2`) are required; `tree` (boolean) sends it to what is below the process too,
the deepest first. The program reads the process's start again right before it sends; if it does not
hold the process is `gone` and nothing is sent. Answers
`{ "outcome", "message", "done", "gone", "denied", "failed" }`: `outcome` is how it came out for the
process named (`done`, `gone`, `denied` where the system refuses, `failed` with the system's words
in `message`); the counts are over the whole tree.

### `renice`

Sets a process's nice value. Params: `pid`, `start` and `nice` (a whole number from -20 to 19). Checked
and answered as `signal` is: `{ "outcome", "message", "done", "gone", "denied", "failed" }`. Lowering
the value below what it is takes root: `denied`.

### `shell`

Lends the pane to the person's shell (`$SHELL`) in a directory; the shell has the terminal until it
ends, and the view is shown again, afresh, after. Params: `cwd` (optional; a directory that is none,
or none given, is home). Answers `{ "ok": true }` before the view goes, and that answer is lost
with the view. Where the terminal cannot lend its pane the call is `unavailable`.

## Topics

### `overview`

The machine as the cards show it. Params: none. The program looks at the machine only while a
subscription is open and the updates are not paused.

The first value is the **history**: the samples the program holds (at most 900), as columns of the
same length, oldest first. A message is held to 1 MiB, so a machine with very many processors is
told the newest that fit.

```json
{ "kind": "history", "interval": 1.0,
  "at": [1761868800], "cpu": [24.1], "cores": [[20, 31, 18]],
  "mem": [58.6], "swap": [20.1], "rx": [216800], "tx": [75700],
  "read": [2300000], "write": [757000], "gpu": [44.0], "gpus": [[44]] }
```

`cpu`, `mem` and `swap` are shares (`swap` is 0 where there is none), `cores` each logical
processor's share in whole percent, `rx`/`tx`/`read`/`write` bytes a second. `gpu` is the busiest
GPU's share (0 where there is none) and `gpus` each GPU's, in whole percent, in the order of the
samples' `gpus` (empty lists where there is none).

Each value after it is one **sample**:

```json
{ "kind": "sample", "at": 1761868920.5,
  "cpu":  { "total": 24.1, "cores": [20.2, 31.0], "freq": [3228, 3228] },
  "mem":  { "total": 17179869184, "used": 10100000000, "available": 7000000000,
            "swapTotal": 4294967296, "swapUsed": 900000000, "pressure": 34.0,
            "breakdown": { "app": 5100000000, "wired": 1900000000, "compressed": 800000000,
                           "cached": 3200000000, "free": 6100000000 } },
  "net":  { "rx": 216800, "tx": 75700, "totalRx": 43000000000, "totalTx": 6400000000,
            "interfaces": [{ "name": "en0", "rx": 210000, "tx": 68000 }] },
  "disk": { "read": 2300000, "write": 757000,
            "devices": [{ "name": "nvme0n1", "read": 2300000, "write": 757000 }],
            "volumes": [{ "name": "/dev/disk3s5", "mount": "/System/Volumes/Data", "fs": "apfs",
                         "total": 530000000000, "used": 323000000000, "percent": 61.0 }] },
  "load": [1.19, 0.96, 0.8], "uptime": 259200,
  "sensors": [{ "name": "CPU performance cores", "celsius": 61.5 }],
  "gpus": [{ "name": "Apple M1 Pro", "load": 44.0, "memoryUsed": 380436480, "memoryTotal": null,
             "celsius": null, "engines": [{ "name": "Renderer", "load": 43.0 }] }] }
```

- `mem.pressure` is `null` where the system does not say; `disk.read`/`write`/`devices` are zero and
  empty where `features.diskIo` is false (it is on macOS, from the storage drivers, and on Linux).
- `net` counts the interfaces that carry traffic of their own (no loopback, no container pairs or
  tunnels); `interfaces` are those that carried something, busiest first, at most 12.
- `disk.volumes` counts a device once however many mount points it has (BTRFS subvolumes, APFS
  volumes of a container), at most 12. **The first is the one the person's files are on.**
- `freq` is each logical processor's frequency, 0 where the system does not say.
- `mem.breakdown` is what memory is made of, bytes that add up to `total` (`null` where the system
  does not say; `wired` and `compressed` are 0 where there is no such thing, as on Linux).
- `sensors` are the temperatures, degrees Celsius, while a card's details are open (`card`); empty
  otherwise, and where the system tells none.
- `gpus` are the graphics processors that say how busy they are, at most 12; **empty where
  `features.gpu` is false.** `load` is 0–100. `memoryUsed` and `memoryTotal` are bytes, `null` where the
  system does not say: `memoryTotal` is also `null` for a GPU that uses the machine's memory (Apple
  Silicon). `celsius` is `null` where the system tells none (with one GPU it may come from the sensors,
  while the card's details are open). `engines` are the parts of the GPU that are busy on their own
  (Apple's renderer and tiler, NVIDIA's memory bus), possibly none.

### `processes`

The rows of the process table that are in view (and 20 more beyond each edge), with the counts to
size the scroll. Params: none. The first message comes with the subscription; a message follows each
sample and each `view` call, so it is told again even while the updates are paused. The table's
state is `view`'s, and each message carries it.

```json
{ "kind": "processes", "at": 1761868920.5,
  "total": 612, "matched": 41, "visible": 41, "moved": 0, "held": false,
  "first": 0, "selectedIndex": 3,
  "rows": [{ "pid": 4120, "start": 1761865000, "ppid": 412, "group": false,
             "name": "Google Chrome", "user": "lantern",
             "command": "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
             "cpu": 12.5, "mem": 838860800, "virt": 4398046511104, "threads": 41, "state": "S",
             "nice": 0, "time": 1032.4, "read": 0, "write": 4096,
             "depth": 1, "children": 0, "folded": false,
             "hits": { "name": [[7, 13]], "command": [[44, 50]] } }],
  "view": { "sort": "cpu", "descending": true, "mode": "flat", "filter": "chrome", "hold": false,
            "root": null, "error": null } }
```

- `total` is every process; `matched` those the filter lets through (below the `root`, where there is
  one); `visible` the rows of the list, which in a tree or in groups are fewer or more than
  `matched`. `first` is the index of `rows[0]` among the `visible` rows.
- `moved` is how many rows are not where the order of the moment would put them, while `held`.
- `selectedIndex` is the index among all rows of the row `selected` names, `null` if it is not in
  the list (it ended, or a folded branch hides it).
- A row's `cpu` is a share of **one** processor (a busy process of many threads passes 100); `mem`
  and `virt` are bytes; `time` is seconds of processor time; `read`/`write` are bytes a second.
  `state` is one letter: `R` running, `S` sleeping, `D` waiting on a disk, `T` stopped, `Z` zombie,
  `I` idle.
- In a tree a folded row carries the totals of what it hides, and `children` counts the processes
  below it; `depth` is how deep it is. A group (`group: true`, `pid` and `start` zero, `name` the
  group's) carries the totals of its members and `children` is how many they are; opened, its
  members follow at `depth` 1. `user` of a group is empty where the members have more than one.
- `exited` (only there when true) is a row of a process that ended, kept one sample while held.
- `hits` (only there when the filter marked something) are ranges of characters `[from, to)` of
  `name` and of `command`.
- A command line is cut at 400 characters; a message is held to 700 KiB by telling fewer rows.

### `process`

The live values of one process, with each sample. Params: `pid` and `start`. The first message comes
with the subscription.

```json
{ "kind": "process", "at": 1761868920.5, "gone": false, "cpu": 12.5, "mem": 838860800,
  "threads": 41, "state": "S", "nice": 0, "time": 1032.4, "read": 0, "write": 4096 }
```

When the process is not in the sample (it ended) `gone` is `true` and nothing else is there. The
page keeps the history it draws from these messages; the program keeps none.

### `ports`

The sockets that listen, with the process that holds each. Params, all optional: `filter` (words
that must all hold, whatever the case: part of the port, protocol, address, process, user or pid;
`:80` is exactly the port 80; at most 200 characters), `sort` (`port`, `address`, `pid`, `process`,
`user`; `port` by default) and `descending`. To change any, subscribe again. The program scans only
while a subscription is open, and not more often than every two seconds; a message is told when
what it says changes.

```json
{ "kind": "ports", "total": 31, "matched": 2,
  "rows": [{ "port": 3000, "proto": "tcp", "addr": "127.0.0.1", "pid": 4120, "start": 1761865000,
             "name": "node", "user": "lantern", "command": "node server.js" }] }
```

- `proto` is `tcp` (a socket that waits for connections) or `udp` (one that is bound to a port);
  `addr` is `*` for every address.
- `pid`, `start`, `name`, `user` and `command` are `null` where the system does not let the person
  see who holds the socket: on Linux the sockets of processes they may not look into, on macOS those
  of processes of other users (run as root to see them).
- At most 500 rows are told; `matched` is how many there were, `total` how many before the filter.
- The `port:8080` of the filter of `view` (`port:80,443`) holds for a process that listens on one of
  those; the program scans for it only while such a filter is set.
