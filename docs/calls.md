# The page and the program

The page (`src/view/`) and the program (`src/program/`) talk only through what is written here:
calls the page makes, and topics it listens to. A change to one starts in this file. The code that
answers is `src/program/api/`; the page's own side is `src/view/parts/overview.lua` and
`src/view/parts/status.lua`.

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
