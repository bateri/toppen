-- The four cards of the overview: CPU, memory, network and disk. A card is a title, the value of
-- this moment, the last minute as a graph, and a line or two of what the value is made of. In a
-- short card the title and the value share a line and the graph is what is left.

local kit = require("../lib/kit")
local graph = require("./graph")
local format = require("../lib/format")

local b, u, S = kit.b, kit.u, kit.S

local cards = {}

local PAD = 12
local GAP = 6
local HEAD = 16
local BIG = 32
local LINE = 15
-- The least a graph is worth drawing at.
local GRAPH_MIN = 28
local STRIP = 14
-- A card shorter than this is compact.
local ROOMY = 190

local function dim(words)
	return b.text(words):color("dim"):truncate():min_w(0)
end

local function quiet(words)
	return b.text(words):color("quiet"):truncate():min_w(0)
end

-- How many samples a graph shows: the last minute, and at least thirty samples.
function cards.window()
	return math.max(30, math.min(kit.SIZE.kept, math.ceil(60 / math.max(S.interval, 0.1))))
end

local function value_text(words, role, size)
	return b.text(words):text_size(size):weight(600):color(role):no_shrink()
end

-- The two lines of a graph that has two (down and up, read and write), in colours of the terminal's
-- own palette: the theme's roles are too near each other to tell two lines apart.
local FIRST = "green"
local SECOND = "magenta"

-- An arrow and a rate: the direction a network or a disk moves bytes in.
local function flow(arrow, role, per_second, size)
	if size < 22 then
		size = 14
	end
	return b.div():row():items_center():gap(3):no_shrink()
		:child(kit.icon(arrow, size - 6, role))
		:child(value_text(format.short_bytes(per_second) .. "/s", "foreground", size))
end

-- The card: `spec` is { title =, side =, big = function(size) -> element, graph = function(w, h) ->
-- element, strip = function(w) -> element (optional), lines = { element, ... } }.
local function card(spec, w, h)
	local roomy = h >= ROOMY
	local inner_w = w - 2 * PAD
	local name = "card/" .. spec.key
	local over = u:hovered(name)
	local box = b.div():w(w):h(h):no_shrink():col():gap(GAP):p(PAD):rounded(8):bg("raised1"):clip()
		:press(name):role("button"):label("Show the details of " .. spec.title)
		:border(1, b.alpha("accent", S.open_card == spec.key and 0.9 or (over and 0.55 or 0)))
	local lines = spec.lines
	local used = 2 * PAD
	local head = b.div():row():items_center():gap(8):h(HEAD):no_shrink()
		:child(b.text(spec.title):weight(600):color("foreground"):no_shrink())
	if spec.side and w >= 220 then
		head:child(b.div():grow(1):min_w(0):child(quiet(spec.side)))
	else
		head:child(b.spacer())
	end
	if roomy then
		box:child(head)
		box:child(b.div():row():items_center():h(BIG):no_shrink():child(spec.big(26)))
		used += HEAD + BIG + 2 * GAP
	else
		head:child(spec.big(18))
		box:child(head)
		lines = { spec.compact or spec.lines[1] }
		used += HEAD + GAP
	end
	used += #lines * LINE + (#lines > 0 and GAP or 0)
	local graph_h = h - used
	local strip = spec.strip and roomy and graph_h - STRIP - GAP >= GRAPH_MIN * 1.5
	if strip then
		graph_h -= STRIP + GAP
	end
	if graph_h >= GRAPH_MIN then
		box:child(spec.graph(inner_w, graph_h))
	else
		box:child(b.spacer())
	end
	if strip then
		box:child(spec.strip(inner_w, STRIP))
	end
	for _, line in lines do
		box:child(b.div():row():items_center():gap(8):h(LINE):no_shrink():child(line))
	end
	return box
end

-- --- The cards ---------------------------------------------------------------------------------------

local function cpu_card()
	local sample = S.sample
	local machine = S.boot.machine
	local total = sample and sample.cpu.total or 0
	local role = format.load_role(total)
	local lines = {}
	if sample then
		local load = sample.load
		lines[#lines + 1] = dim(string.format("load %.2f %.2f %.2f", load[1], load[2], load[3]))
		local sum, count = 0, 0
		for _, freq in sample.cpu.freq do
			if freq > 0 then
				sum += freq
				count += 1
			end
		end
		if count > 0 then
			lines[#lines + 1] = quiet(string.format("%d threads · %.1f GHz", machine.threads, sum / count / 1000))
		else
			lines[#lines + 1] = quiet(string.format("%d threads", machine.threads))
		end
	end
	return {
		key = "cpu",
		title = "CPU",
		side = machine.cpu,
		big = function(size)
			return value_text(format.percent(total), role, size)
		end,
		graph = function(w, h)
			return graph.area({ { values = S.series.cpu, role = role } }, cards.window(), 100, w, h)
		end,
		strip = machine.threads > 1 and sample and function(w, h)
			return graph.bars(sample.cpu.cores, "accent", w, h)
		end or nil,
		lines = lines,
	}
end

local function memory_card()
	local sample = S.sample
	local mem = sample and sample.mem
	local share = mem and mem.total > 0 and mem.used / mem.total * 100 or 0
	local role = format.load_role(share)
	local lines = {}
	if mem then
		lines[#lines + 1] = dim(string.format("%s of %s", format.bytes(mem.used), format.bytes(mem.total)))
		local second = {}
		if mem.swapTotal > 0 then
			second[#second + 1] = string.format("swap %s of %s", format.bytes(mem.swapUsed), format.bytes(mem.swapTotal))
		end
		if mem.pressure ~= nil then
			second[#second + 1] = string.format("pressure %d%%", math.floor(mem.pressure + 0.5))
		end
		if #second > 0 then
			lines[#lines + 1] = quiet(table.concat(second, " · "))
		end
	end
	return {
		key = "memory",
		title = "Memory",
		big = function(size)
			return value_text(format.percent(share), role, size)
		end,
		graph = function(w, h)
			local count = cards.window()
			local drawn = { { values = S.series.mem, role = role } }
			if mem and mem.swapTotal > 0 then
				drawn[#drawn + 1] = { values = S.series.swap, role = SECOND }
			end
			return graph.area(drawn, count, 100, w, h)
		end,
		lines = lines,
	}
end

local function network_card()
	local sample = S.sample
	local net = sample and sample.net
	local lines = {}
	local compact = nil
	if net then
		lines[#lines + 1] = dim(string.format("%s down · %s up since boot", format.bytes(net.totalRx), format.bytes(net.totalTx)))
		compact = dim(string.format("%s down · %s up", format.bytes(net.totalRx), format.bytes(net.totalTx)))
		local busiest = net.interfaces[1]
		if busiest then
			lines[#lines + 1] = quiet(busiest.name)
		end
	end
	return {
		key = "network",
		title = "Network",
		compact = compact,
		big = function(size)
			local row = b.div():row():items_center():gap(10):no_shrink()
			row:child(flow("arrow-down", FIRST, net and net.rx or 0, size))
			row:child(flow("arrow-up", SECOND, net and net.tx or 0, size))
			return row
		end,
		graph = function(w, h)
			local count = cards.window()
			local lines_drawn = { { values = S.series.rx, role = FIRST }, { values = S.series.tx, role = SECOND } }
			return graph.area(lines_drawn, count, graph.ceiling(lines_drawn, count, 10 * 1024) * 1.1, w, h)
		end,
		lines = lines,
	}
end

local function disk_card()
	local sample = S.sample
	local disk = sample and sample.disk
	local io = S.boot.features.diskIo
	-- The program tells the volume the person's files are on first.
	local volume = disk and disk.volumes[1]
	local lines = {}
	if volume then
		lines[#lines + 1] = dim(string.format("%s · %s of %s", volume.mount, format.bytes(volume.used), format.bytes(volume.total)))
	end
	if disk and #disk.volumes > 1 then
		lines[#lines + 1] = quiet(string.format("%d volumes", #disk.volumes))
	end
	local spec = { key = "disk", title = "Disk", lines = lines }
	if volume then
		spec.compact = dim(string.format("%s of %s · %s", format.bytes(volume.used), format.bytes(volume.total), format.percent(volume.total > 0 and volume.used / volume.total * 100 or 0)))
	end
	if io then
		spec.big = function(size)
			local row = b.div():row():items_center():gap(10):no_shrink()
			row:child(flow("arrow-down", FIRST, disk and disk.read or 0, size))
			row:child(flow("arrow-up", SECOND, disk and disk.write or 0, size))
			return row
		end
		spec.graph = function(w, h)
			local count = cards.window()
			local drawn = { { values = S.series.read, role = FIRST }, { values = S.series.write, role = SECOND } }
			return graph.area(drawn, count, graph.ceiling(drawn, count, 100 * 1024) * 1.1, w, h)
		end
	else
		local share = volume and volume.total > 0 and volume.used / volume.total * 100 or 0
		spec.big = function(size)
			return value_text(format.percent(share), format.load_role(share), size)
		end
		spec.graph = function(w, h)
			-- The system measures no reads and writes here: the volumes are what there is to show.
			local box = b.div():w(w):h(h):no_shrink():col():gap(8):justify_center()
			local shown = 0
			if disk then
				for _, one in disk.volumes do
					if shown >= math.max(1, math.floor(h / 26)) then
						break
					end
					shown += 1
					local used = one.total > 0 and one.used / one.total or 0
					box:child(b.div():row():items_center():gap(8):h(18):no_shrink()
						:child(b.text(one.mount):color("dim"):truncate():min_w(0):w(math.floor(w * 0.4)))
						:child(graph.meter(used, format.load_role(used * 100), math.floor(w * 0.6) - 8, 6)))
				end
			end
			return box
		end
	end
	return spec
end

-- --- The gauges -------------------------------------------------------------------------------------------

local GAUGE = 20
local NAME_W = 64

-- The four cards as four lines, for a pane too narrow for a graph each: a name, a meter and the
-- value of this moment, in `w` points. Network has no meter: its rates are not a share of anything.
function cards.gauges(w)
	local sample = S.sample
	local value_w = w >= 420 and 168 or 118
	local box = b.div():col():gap(2):w(w):no_shrink()
	local function line(name, share, words, key)
		local row = b.div():row():items_center():gap(8):h(GAUGE):w(w):no_shrink()
			:press("card/" .. key):role("button"):label("Show the details of " .. name)
		row:child(b.text(name):weight(600):w(NAME_W):no_shrink())
		if share then
			row:child(graph.meter(share / 100, format.load_role(share), math.max(w - NAME_W - value_w - 16, 24), 6))
		else
			row:child(b.spacer())
		end
		row:child(b.text(words):color("dim"):truncate():min_w(0):w(share and value_w or w - NAME_W - 8))
		box:child(row)
	end
	if not sample then
		return box
	end
	line("CPU", sample.cpu.total, string.format("%s  load %.2f", format.percent(sample.cpu.total), sample.load[1]), "cpu")
	local mem = sample.mem
	local used = mem.total > 0 and mem.used / mem.total * 100 or 0
	line("Memory", used, string.format("%s  %s", format.percent(used), format.short_bytes(mem.used)), "memory")
	line("Network", nil, string.format("down %s · up %s", format.rate(sample.net.rx), format.rate(sample.net.tx)), "network")
	local volume = sample.disk.volumes[1]
	if volume then
		local share = volume.total > 0 and volume.used / volume.total * 100 or 0
		line("Disk", share, string.format("%s  %s", format.percent(share), volume.mount), "disk")
	end
	return box
end

-- The four cards at `w` by `h` points each, in the order of the overview.
function cards.list(w, h)
	return {
		card(cpu_card(), w, h),
		card(memory_card(), w, h),
		card(network_card(), w, h),
		card(disk_card(), w, h),
	}
end

return cards
