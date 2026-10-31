-- The details of a card of the overview: the machine's CPU, memory, network or disk at length, in
-- place of the table until the person goes back. Each is drawn from what the overview already told
-- (the series of the page, the last sample); the program is told which card is open only so that it
-- reads the temperature sensors while one is (`card`).
--
-- A graph says what it held at the place the pointer rests on, and how long ago that was. The time
-- the graphs span is the person's: a minute, five or fifteen.

local kit = require("../lib/kit")
local format = require("../lib/format")
local graph = require("./graph")

local b, u, S, SIZE = kit.b, kit.u, kit.S, kit.SIZE

local machine = {}

local PAD = 14
local FIRST = "green"
local SECOND = "magenta"

local RANGES = {
	{ key = "60", title = "1 min" },
	{ key = "300", title = "5 min" },
	{ key = "900", title = "15 min" },
}

local CARDS = { "cpu", "memory", "network", "disk" }
local TITLES = { cpu = "CPU", memory = "Memory", network = "Network", disk = "Disk" }

local M = {
	-- The card open: "cpu", "memory", "network", "disk", or nil.
	open = nil,
	-- Seconds the graphs span.
	range = 60,
	-- The interface the network graph is of: its name, "" for all of them.
	iface = "",
	-- Where the pointer rests on a graph: { name =, frac = } (0 at its left edge, 1 at its right).
	hover = nil,
	-- The graphs drawn at the last render, by name, for the pointer to find.
	graphs = {},
}
machine.state = M

-- --- Opening and closing --------------------------------------------------------------------------

local function tell()
	kit.call("card", { card = M.open ~= nil and M.open or b.json.null }, function()
		return true
	end)
end

function machine.show(card)
	if M.open == card then
		return
	end
	M.open = card
	S.open_card = card
	M.hover = nil
	tell()
end

function machine.close()
	if M.open == nil then
		return
	end
	M.open = nil
	S.open_card = nil
	M.hover = nil
	tell()
	kit.gather(u:focus(u:name("procs", "body")))
end

-- The key of the card: opens it, or closes it where it is open.
function machine.toggle(card)
	if M.open == card then
		machine.close()
	else
		machine.show(card)
	end
end

function machine.next_range()
	local at = 1
	for index, one in RANGES do
		if tonumber(one.key) == M.range then
			at = index
		end
	end
	M.range = tonumber(RANGES[at % #RANGES + 1].key)
end

-- --- What a graph shows -----------------------------------------------------------------------------------

-- How many samples a graph spans: the range, at this interval.
local function count()
	return math.max(10, math.min(SIZE.kept, math.ceil(M.range / math.max(S.interval, 0.1))))
end

-- How many samples back from the newest the pointer is at on graph `name`, or nil where it is not
-- over it. `values` is how many samples there are.
local function back_of(name, w)
	local hover = M.hover
	if hover == nil or hover.name ~= name then
		return nil
	end
	local step = w / math.max(count() - 1, 1)
	return math.max(0, math.floor((1 - hover.frac) * w / step + 0.5))
end

-- The time of `back` samples ago, in words.
local function ago(back)
	if back == 0 then
		return "now"
	end
	return format.span(back * S.interval) .. " ago"
end

-- A graph of `lines` that the pointer can rest on: the box is named `name`. Returns the graph and the
-- number of samples back the pointer is at (nil when it is not on it).
local function plot(name, lines, ceiling, w, h)
	M.graphs[name] = true
	local back = back_of(name, w)
	local body = graph.area(lines, count(), ceiling, w, h, back)
	return b.div():id(name):w(w):h(h):no_shrink():rounded(4):bg("raised1"):clip():child(body), back
end

-- The value of a series `back` samples ago, or nil.
local function at_back(values, back)
	return values[#values - back]
end

-- A head for a graph: its title on the left, and on the right either the readout of the moment the
-- pointer is at (with how long ago that was) or `now`.
local function caption(title, now, readout, back)
	local row = b.div():row():items_center():gap(10):min_w(0)
	row:child(b.text(title):color("quiet"):text_size(12):weight(600):no_shrink())
	row:child(b.spacer())
	if back ~= nil then
		row:child(b.text(ago(back)):color("quiet"):text_size(12):no_shrink())
		if readout ~= nil then
			row:child(readout)
		end
	elseif now ~= nil then
		row:child(now)
	end
	return row
end

local function tone(words, role)
	return b.text(words):weight(600):color(role):no_shrink()
end

local function two(first, second)
	local row = b.div():row():items_center():gap(10):no_shrink()
	row:child(first)
	row:child(second)
	return row
end

-- The pointer moved over the page: where it rests on a graph. Whether that changed.
function machine.pointer(x, y)
	if M.open == nil then
		return false
	end
	local found = nil
	for name in M.graphs do
		local box = b.layout.bounds(name)
		if box ~= nil and x >= box.x and x < box.x + box.w and y >= box.y and y < box.y + box.h then
			local frac = math.min(math.max((x - box.x) / math.max(box.w - 1, 1), 0), 1)
			if M.hover ~= nil and M.hover.name == name and math.abs(M.hover.frac - frac) < 0.002 then
				return false
			end
			found = { name = name, frac = frac }
			break
		end
	end
	if found == nil and M.hover == nil then
		return false
	end
	M.hover = found
	return true
end

function machine.leave()
	if M.hover ~= nil then
		M.hover = nil
		return true
	end
	return false
end

-- --- Pieces ------------------------------------------------------------------------------------------------------

local function title_row(card, now)
	local row = b.div():row():items_center():gap(12):min_w(0)
	row:child(b.text(TITLES[card]):text_size(16):weight(650):no_shrink())
	for _, piece in now do
		row:child(piece)
	end
	row:child(b.spacer())
	if S.width >= 520 then
		row:child(u:segments("range", { value = tostring(M.range), items = RANGES }))
	end
	row:child(u:icon_button("machine-close", { icon = "close", label = "Back to the processes", keys = "escape" }))
	return row
end

local function quiet(words)
	return b.text(words):color("quiet"):text_size(12):truncate():min_w(0)
end

local function dim(words)
	return b.text(words):color("dim"):truncate():min_w(0)
end

local function label(words)
	return b.text(words):text_size(11):color("quiet"):weight(600)
end

-- A line of a list: a name, a meter and a value.
local function meter_row(name, share, words, inner, name_w)
	local row = b.div():row():items_center():gap(10):h(22):w(inner):no_shrink()
	row:child(b.text(name):truncate():min_w(0):w(name_w):no_shrink())
	row:child(graph.meter(share, format.load_role(share * 100), math.max(inner - name_w - 190, 40), 6))
	row:child(b.text(words):color("dim"):text_size(12):truncate():min_w(0):grow(1))
	return row
end

-- --- CPU ---------------------------------------------------------------------------------------------------------

local function cpu_view(inner)
	local sample = S.sample
	local machine_info = S.boot.machine
	local total = sample and sample.cpu.total or 0
	local role = format.load_role(total)
	local now = { tone(format.percent(total), role) }
	if sample then
		now[#now + 1] = dim(string.format("load %.2f %.2f %.2f", sample.load[1], sample.load[2], sample.load[3]))
		local sum, n = 0, 0
		for _, freq in sample.cpu.freq do
			if freq > 0 then
				sum += freq
				n += 1
			end
		end
		if n > 0 then
			now[#now + 1] = quiet(string.format("%.1f GHz", sum / n / 1000))
		end
	end
	local box = b.div():col():gap(14):p(PAD):w(inner + 2 * PAD):no_shrink()
	box:child(title_row("cpu", now))
	box:child(quiet(string.format("%s · %d threads · %d cores", machine_info.cpu, machine_info.threads, machine_info.cores)))

	local lines = { { values = S.series.cpu, role = role } }
	local graph_box, back = plot("graph/cpu", lines, 100, inner, 110)
	local at = back and at_back(S.series.cpu, back)
	box:child(caption("Processor use", nil, at and tone(string.format("%.1f%%", at), role), back))
	box:child(graph_box)

	-- Each logical processor.
	local cores = sample and sample.cpu.cores or {}
	if #cores > 1 then
		box:child(label(string.format("Processors · %d", #cores)))
		local gap = 8
		-- As many to a row as fit, and as even as the rows can be: eight in two rows of four, not seven and one.
		local fit = math.max(1, math.floor((inner + gap) / (150 + gap)))
		local columns = math.ceil(#cores / math.ceil(#cores / fit))
		local cell_w = math.floor((inner - gap * (columns - 1)) / columns)
		local grid = b.div():row():wrap():gap(gap):w(inner)
		for index, share in cores do
			local freq = sample.cpu.freq[index]
			local head = b.div():row():items_center():gap(6):min_w(0)
			head:child(b.text(tostring(index - 1)):color("quiet"):text_size(11):w(22):no_shrink())
			head:child(b.text(format.percent(share)):weight(600):color(format.load_role(share)):no_shrink())
			head:child(b.spacer())
			if freq ~= nil and freq > 0 and cell_w >= 130 then
				head:child(quiet(string.format("%.1f GHz", freq / 1000)))
			end
			local cell = b.div():col():gap(4):w(cell_w):no_shrink():p(6):rounded(6):bg("raised1"):clip()
			cell:child(head)
			cell:child(graph.area({ { values = S.cores[index] or {}, role = format.load_role(share) } }, count(), 100, cell_w - 12, 26))
			grid:child(cell)
		end
		box:child(grid)
	end

	-- The temperatures.
	box:child(label("Temperatures"))
	local sensors = sample and sample.sensors or {}
	if #sensors == 0 then
		box:child(quiet("None told yet. Where this machine tells none, none will be."))
	else
		for _, sensor in sensors do
			local hot = sensor.celsius
			box:child(meter_row(sensor.name, math.min(hot / 100, 1), string.format("%.1f °C", hot), inner, 220))
		end
	end
	return box
end

-- --- Memory ------------------------------------------------------------------------------------------------------

local PARTS = {
	{ key = "app", title = "App", role = "accent" },
	{ key = "wired", title = "Wired", role = "magenta" },
	{ key = "compressed", title = "Compressed", role = "yellow" },
	{ key = "cached", title = "Cached files", role = "green" },
	{ key = "free", title = "Free", role = nil },
}

-- A small square of a colour and its word.
local function swatched(role, words)
	local item = b.div():row():items_center():gap(6):no_shrink()
	item:child(b.div():w(10):h(10):rounded(2):no_shrink():bg(role))
	item:child(b.text(words):color("dim"):text_size(12):no_shrink())
	return item
end

local function stack(parts, total, inner)
	local bar = b.div():row():w(inner):h(16):no_shrink():rounded(4):clip():bg(b.alpha("foreground", 0.08))
	for _, part in PARTS do
		local bytes = parts[part.key] or 0
		if part.role ~= nil and bytes > 0 and total > 0 then
			local width = math.max(math.floor(inner * bytes / total), 1)
			bar:child(b.div():w(width):h(16):no_shrink():bg(part.role))
		end
	end
	return bar
end

local function memory_view(inner)
	local sample = S.sample
	local mem = sample and sample.mem
	local share = mem and mem.total > 0 and mem.used / mem.total * 100 or 0
	local role = format.load_role(share)
	local now = { tone(format.percent(share), role) }
	if mem then
		now[#now + 1] = dim(string.format("%s of %s", format.bytes(mem.used), format.bytes(mem.total)))
		if mem.pressure ~= nil then
			now[#now + 1] = quiet(string.format("pressure %d%%", math.floor(mem.pressure + 0.5)))
		end
	end
	local box = b.div():col():gap(14):p(PAD):w(inner + 2 * PAD):no_shrink()
	box:child(title_row("memory", now))

	if mem then
		local parts = mem.breakdown
		if parts == nil then
			parts = { app = mem.used, free = math.max(mem.total - mem.used, 0) }
		end
		box:child(label("What it is made of"))
		box:child(stack(parts, mem.total, inner))
		local legend = b.div():row():wrap():gap_x(16):gap_y(4):w(inner)
		for _, part in PARTS do
			local bytes = parts[part.key]
			if bytes ~= nil and (bytes > 0 or part.key == "app" or part.key == "free") then
				local swatch = b.div():w(10):h(10):rounded(2):no_shrink()
				swatch:bg(part.role or b.alpha("foreground", 0.2))
				local item = b.div():row():items_center():gap(6):no_shrink()
				item:child(swatch)
				item:child(b.text(part.title):color("dim"):no_shrink())
				item:child(b.text(format.bytes(bytes)):no_shrink())
				legend:child(item)
			end
		end
		box:child(legend)
	end

	local lines = { { values = S.series.mem, role = role } }
	local has_swap = mem and mem.swapTotal > 0
	if has_swap then
		lines[#lines + 1] = { values = S.series.swap, role = SECOND }
	end
	local graph_box, back = plot("graph/memory", lines, 100, inner, 110)
	local readout = nil
	if back ~= nil then
		local used = at_back(S.series.mem, back)
		local swap = has_swap and at_back(S.series.swap, back) or nil
		if used ~= nil then
			readout = two(tone(string.format("%.1f%% used", used), role), swap and tone(string.format("%.1f%% swap", swap), SECOND) or b.spacer())
		end
	end
	local key = nil
	if has_swap then
		key = two(swatched(role, "memory"), swatched(SECOND, "swap"))
	end
	box:child(caption("Memory in use", key, readout, back))
	box:child(graph_box)
	if has_swap then
		box:child(dim(string.format("Swap %s of %s", format.bytes(mem.swapUsed), format.bytes(mem.swapTotal))))
	end
	return box
end

-- --- Network -----------------------------------------------------------------------------------------------------

-- The interfaces the page has seen carry something, busiest now first.
local function interface_names()
	local rates = {}
	if S.sample then
		for _, one in S.sample.net.interfaces do
			rates[one.name] = one.rx + one.tx
		end
	end
	local names = {}
	for name in S.ifaces do
		names[#names + 1] = name
	end
	table.sort(names, function(a, c)
		local ra, rc = rates[a] or 0, rates[c] or 0
		if ra ~= rc then
			return ra > rc
		end
		return a < c
	end)
	return names
end

-- Moves the choice of interface by `step` places through "all" and the names.
function machine.step_interface(step)
	local names = interface_names()
	table.insert(names, 1, "")
	local at = 1
	for index, name in names do
		if name == M.iface then
			at = index
		end
	end
	M.iface = names[math.max(1, math.min(#names, at + step))]
end

local function net_row(name, rx, tx, chosen, w)
	local id = "iface/" .. name
	local over = u:hovered(id)
	local row = b.div():row():items_center():gap(8):h(40):w(w):no_shrink():px(8):rounded(6)
		:press(id):role("button"):label(name == "" and "All interfaces" or name)
		:bg(chosen and b.alpha("accent", 0.2) or (over and b.alpha("foreground", 0.07) or nil))
	row:child(b.text(name == "" and "All interfaces" or name):weight(chosen and 650 or 400):truncate():min_w(0):grow(1))
	local rates = b.div():col():items_end():no_shrink()
	rates:child(b.text("↓ " .. format.rate(rx)):color(FIRST):text_size(12))
	rates:child(b.text("↑ " .. format.rate(tx)):color(SECOND):text_size(12))
	row:child(rates)
	return row
end

local function network_view(inner)
	local sample = S.sample
	local net = sample and sample.net
	local now = {}
	if net then
		now[1] = two(
			tone("↓ " .. format.rate(net.rx), FIRST),
			tone("↑ " .. format.rate(net.tx), SECOND)
		)
		now[2] = dim(string.format("%s down · %s up since boot", format.bytes(net.totalRx), format.bytes(net.totalTx)))
	end
	local box = b.div():col():gap(14):p(PAD):w(inner + 2 * PAD):no_shrink()
	box:child(title_row("network", now))

	local rx_series, tx_series = S.series.rx, S.series.tx
	local lists = S.ifaces[M.iface]
	if M.iface ~= "" and lists == nil then
		M.iface = ""
	end
	if M.iface ~= "" then
		rx_series, tx_series = lists.rx, lists.tx
	end

	local narrow = inner < 560
	local list_w = narrow and inner or 230
	local graph_w = narrow and inner or inner - list_w - 16

	-- The interfaces.
	local rates = {}
	if net then
		for _, one in net.interfaces do
			rates[one.name] = one
		end
	end
	local list = b.div():col():gap(2):w(list_w):no_shrink()
	list:child(label("Interfaces"))
	list:child(net_row("", net and net.rx or 0, net and net.tx or 0, M.iface == "", list_w))
	for _, name in interface_names() do
		local one = rates[name]
		list:child(net_row(name, one and one.rx or 0, one and one.tx or 0, M.iface == name, list_w))
	end

	-- What the chosen one did.
	local lines = { { values = rx_series, role = FIRST }, { values = tx_series, role = SECOND } }
	local ceiling = graph.ceiling(lines, count(), 10 * 1024) * 1.1
	local graph_box, back = plot("graph/network", lines, ceiling, graph_w, narrow and 120 or 200)
	local readout = nil
	if back ~= nil then
		local rx, tx = at_back(rx_series, back), at_back(tx_series, back)
		if rx ~= nil and tx ~= nil then
			readout = two(tone("↓ " .. format.rate(rx), FIRST), tone("↑ " .. format.rate(tx), SECOND))
		end
	end
	local right = b.div():col():gap(8):w(graph_w):no_shrink()
	right:child(caption(M.iface == "" and "All interfaces" or M.iface, nil, readout, back))
	right:child(graph_box)
	right:child(quiet(string.format("Scale up to %s/s", format.short_bytes(ceiling))))

	local body = b.div():gap(16):w(inner):no_shrink()
	if narrow then
		body:col():child(list):child(right)
	else
		body:row():child(list):child(right)
	end
	box:child(body)
	return box
end

-- --- Disk ----------------------------------------------------------------------------------------------------------

local function disk_view(inner)
	local sample = S.sample
	local disk = sample and sample.disk
	local io = S.boot.features.diskIo
	local now = {}
	if io and disk then
		now[1] = two(tone("↓ " .. format.rate(disk.read), FIRST), tone("↑ " .. format.rate(disk.write), SECOND))
	end
	local box = b.div():col():gap(14):p(PAD):w(inner + 2 * PAD):no_shrink()
	box:child(title_row("disk", now))

	if io then
		local lines = { { values = S.series.read, role = FIRST }, { values = S.series.write, role = SECOND } }
		local ceiling = graph.ceiling(lines, count(), 100 * 1024) * 1.1
		local graph_box, back = plot("graph/disk", lines, ceiling, inner, 110)
		local readout = nil
		if back ~= nil then
			local read, write = at_back(S.series.read, back), at_back(S.series.write, back)
			if read ~= nil and write ~= nil then
				readout = two(tone("read " .. format.rate(read), FIRST), tone("write " .. format.rate(write), SECOND))
			end
		end
		box:child(caption("Reads and writes", quiet(string.format("scale up to %s/s", format.short_bytes(ceiling))), readout, back))
		box:child(graph_box)
	else
		box:child(quiet("This system does not tell how much the disks read and write."))
	end

	-- The volumes, the one the person's files are on first.
	if disk and #disk.volumes > 0 then
		box:child(label(string.format("Volumes · %d", #disk.volumes)))
		for _, volume in disk.volumes do
			local share = volume.total > 0 and volume.used / volume.total or 0
			local card = b.div():col():gap(4):w(inner):no_shrink():p(8):rounded(6):bg("raised1")
			local head = b.div():row():items_center():gap(10):min_w(0)
			head:child(b.text(volume.mount):weight(600):truncate():min_w(0))
			head:child(quiet(volume.fs))
			head:child(b.spacer())
			head:child(b.text(format.percent(share * 100)):weight(600):color(format.load_role(share * 100)):no_shrink())
			card:child(head)
			card:child(graph.meter(share, format.load_role(share * 100), inner - 16, 6))
			card:child(quiet(string.format("%s of %s · %s", format.bytes(volume.used), format.bytes(volume.total), volume.name)))
			box:child(card)
		end
	end
	if io and disk and #disk.devices > 0 then
		box:child(label("Devices"))
		for _, device in disk.devices do
			local row = b.div():row():items_center():gap(10):h(22):w(inner):no_shrink()
			row:child(b.text(device.name):truncate():min_w(0):grow(1))
			row:child(b.text("read " .. format.rate(device.read)):color(FIRST):text_size(12):no_shrink())
			row:child(b.text("write " .. format.rate(device.write)):color(SECOND):text_size(12):no_shrink())
			box:child(row)
		end
	end
	return box
end

-- --- The view ------------------------------------------------------------------------------------------------

local VIEWS = { cpu = cpu_view, memory = memory_view, network = network_view, disk = disk_view }

-- What takes the place of the table while a card is open. `nil` when none is.
function machine.view()
	if M.open == nil then
		return nil
	end
	M.graphs = {}
	local inner = math.max(S.width - 2 * PAD - 12, 200)
	local body = VIEWS[M.open](inner)
	return b.div():id("machine/body"):scroll():grow(1):min_h(0):child(body)
end

-- An action of the view: the range, the way back, an interface chosen.
function machine.action(action)
	local from, kind = action.from, action.kind
	if from == "range" and kind == "select" then
		M.range = tonumber(action.key) or M.range
		return true
	elseif from == "machine-close" and kind == "press" then
		machine.close()
		return true
	end
	return nil
end

-- A press no component took: a card, or an interface in the list.
function machine.event(event)
	if event.type ~= "press" or (event.button or "left") ~= "left" then
		return nil
	end
	local over = b.layout.hit({ x = event.x, y = event.y })
	if over == nil then
		return nil
	end
	local card = string.match(over, "^card/(%a+)$")
	if card ~= nil then
		machine.toggle(card)
		return true
	end
	local iface = string.match(over, "^iface/(.*)$")
	if iface ~= nil and M.open == "network" then
		M.iface = iface
		return true
	end
	return nil
end

-- A key of the view: the arrows choose an interface where there is a list of them.
function machine.key(event)
	if M.open ~= "network" then
		return nil
	end
	if event.key == "down" then
		machine.step_interface(1)
		return true
	elseif event.key == "up" then
		machine.step_interface(-1)
		return true
	end
	return nil
end

machine.CARDS = CARDS
machine.TITLES = TITLES

return machine
