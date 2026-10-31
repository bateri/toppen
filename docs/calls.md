# The page and the program

The page (`src/view/`) and the program (`src/program/`) talk only through what is written here:
calls the page makes, and topics it listens to. A change to one starts in this file. The code that
answers is `src/program/api/`; the page's own side is `src/view/parts/` (`overview.lua`, `table.lua`,
`filter.lua`, `status.lua`).

**Units.** Bytes are bytes (1024 to the step when a person reads them), a share is a number from 0 to
100, time is seconds, a rate is bytes a second, a frequency is MHz. Members are `camelCase`.

**Errors.** A call that cannot be done answers with an error: `bad-params` (the params are not what
the call takes: a member it does not know, a value out of range; the words say which), `not-found`
(no such call), `internal` (the program failed). The words of an error are for the person.

## Calls

### `boot`

What the page needs before it draws, once. Params: none.

```json
{
  "version": "0.1.0",
  "machine": {
    "host": "lantern", "os": "macOS 26.4.1", "kernel": "25.4.0", "arch": "arm64",
    "cpu": "Apple M1 Pro", "threads": 10, "cores": 10,
    "memory": 34359738368, "booted": 1761600000
  },
  "features": { "pressure": true, "diskIo": false },
  "sampling": { "interval": 1.0, "paused": false, "min": 0.5, "max": 10.0 }
}
```

`features` says what this machine can measure: a card whose reading is not there is drawn without
it, not with a zero.

- `pressure`: the system's own measure of how pressed memory is (macOS's memory pressure, Linux's PSI).
- `diskIo`: reads and writes of the disks, in bytes a second.

### `sampling`

How often the machine is looked at, and whether it is. Params: `interval` (seconds, 0.5 to 10) and
`paused` (boolean), both optional. Answers what is now so: `{ "interval", "paused", "min", "max" }`.
While paused the program looks at nothing.

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
  "read": [2300000], "write": [757000] }
```

`cpu`, `mem` and `swap` are shares (`swap` is 0 where there is none), `cores` each logical
processor's share in whole percent, `rx`/`tx`/`read`/`write` bytes a second.

Each value after it is one **sample**:

```json
{ "kind": "sample", "at": 1761868920.5,
  "cpu":  { "total": 24.1, "cores": [20.2, 31.0], "freq": [3228, 3228] },
  "mem":  { "total": 17179869184, "used": 10100000000, "available": 7000000000,
            "swapTotal": 4294967296, "swapUsed": 900000000, "pressure": 34.0 },
  "net":  { "rx": 216800, "tx": 75700, "totalRx": 43000000000, "totalTx": 6400000000,
            "interfaces": [{ "name": "en0", "rx": 210000, "tx": 68000 }] },
  "disk": { "read": 2300000, "write": 757000,
            "devices": [{ "name": "nvme0n1", "read": 2300000, "write": 757000 }],
            "volumes": [{ "name": "/dev/disk3s5", "mount": "/System/Volumes/Data", "fs": "apfs",
                         "total": 530000000000, "used": 323000000000, "percent": 61.0 }] },
  "load": [1.19, 0.96, 0.8], "uptime": 259200 }
```

- `mem.pressure` is `null` where the system does not say; `disk.read`/`write`/`devices` are zero and
  empty where `features.diskIo` is false.
- `net` counts the interfaces that carry traffic of their own (no loopback, no container pairs or
  tunnels); `interfaces` are those that carried something, busiest first, at most 12.
- `disk.volumes` counts a device once however many mount points it has (BTRFS subvolumes, APFS
  volumes of a container), at most 12. **The first is the one the person's files are on.**
- `freq` is each logical processor's frequency, 0 where the system does not say.

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
