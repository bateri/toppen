-- The cards of the overview: CPU, memory, network and disk, and the GPU where the machine has one.
-- Each card has a colour of its own (its title, its graph, the ground of its graph): a caption, the
-- value of this moment large with its unit small beside it, three figures of what the value is made
-- of, and the last minute as a matrix of dots. A shorter card leaves the caption out; a short one
-- puts the value on the line of its title and its figures on one line under its graph.

local kit = require("../lib/kit")
local graph = require("./graph")
local format = require("../lib/format")

local b, u, S = kit.b, kit.u, kit.S

local cards = {}

local PAD = 14
local GAP = 6
local HEAD = 18
local CAPTION = 14
local BIG = 34
local FIGURES = 32
local LINE = 16
-- The least a graph is worth drawing at.
local GRAPH_MIN = 24
local STRIP = 12
-- A card at least this tall has its caption; one at least `ROOMY` tall its figures on their own
-- line; a shorter one is compact.
local FULL = 196
local ROOMY = 150

-- The colour of each card, and the second line of the cards that draw two: the terminal's own
-- palette, whose colours are far enough apart to tell the cards and the two lines apart.
local ROLES = { cpu = "blue", memory = "green", network = "cyan", disk = "yellow", gpu = "magenta" }
local SECOND = "magenta"
local SECOND_DISK = "red"

local function quiet(words)
	return b.text(words):color("quiet"):truncate():min_w(0)
end

-- How many samples a graph shows: the last minute, and at least thirty samples.
function cards.window()
	return math.max(30, math.min(kit.SIZE.kept, math.ceil(60 / math.max(S.interval, 0.1))))
end

-- A number of bytes as its number and its unit, apart: "10.2", "GiB"; a whole number from a
-- hundred on ("193", not "193.0").
local function bytes_apart(count, per_second)
	local words = format.bytes(count)
	local number, unit = string.match(words, "^(%S+)%s+(%S+)$")
	if number == nil then
		return words, ""
	end
	if tonumber(number) ~= nil and tonumber(number) >= 100 then
		number = string.format("%d", math.floor(tonumber(number) + 0.5))
	end
	return number, per_second and (unit .. "/s") or unit
end

-- A size and a rate as a figure under the value says them: three characters of number at most.
local short = format.short_bytes
local function short_rate(per_second)
	return format.short_bytes(per_second) .. "/s"
end

-- The last part of a mount point, for the side of a narrow card: "Data", not "/System/Volumes/Data".
local function mount_name(mount)
	if mount == "/" then
		return mount
	end
	return string.match(mount, "([^/]+)/*$") or mount
end

-- The value of this moment: the number large, its unit small and dim beside it, on one baseline.
local function value_of(number, unit, role, size)
	local row = b.div():row():items_end():gap(3):no_shrink()
		:child(b.text(number):text_size(size):weight(650):color(role):no_shrink())
	if unit ~= "" then
		row:child(b.text(unit):text_size(math.max(math.floor(size * 0.5), 11)):color("dim"):no_shrink()
			:pb(math.floor(size * 0.12)))
	end
	return row
end

-- The figures under the value: up to three, each a quiet label over its value.
local function figures(list, w)
	local row = b.div():row():gap(8):h(FIGURES):w(w):no_shrink()
	-- A narrow card says its figures smaller, so that "673 KiB/s" fits its column.
	local column = (w - 8 * math.max(#list - 1, 0)) / math.max(#list, 1)
	if column < 66 and #list > 2 then
		-- Too narrow for three: two whole figures say more than three cut ones.
		list = { list[1], list[2] }
		column = (w - 8) / 2
	end
	local size = column >= 80 and 12.5 or 11
	for _, figure in list do
		row:child(b.div():col():gap(2):grow(1):w(0):min_w(0)
			:child(b.text(figure[1]):text_size(10.5):color("quiet"):truncate():min_w(0))
			:child(b.text(figure[2]):text_size(size):weight(600):truncate():min_w(0)))
	end
	return row
end

-- The figures on one line, for a compact card.
local function figures_line(list)
	local parts = {}
	for _, figure in list do
		parts[#parts + 1] = string.lower(figure[1]) .. " " .. figure[2]
	end
	return quiet(table.concat(parts, " · "))
end

-- The card: `spec` is { key =, title =, side =, caption =, number =, unit =, value_role =, badge =
-- { words, tone } or nil, figures = { { label, value } x3 }, graph = function(w, h) -> element,
-- strip = function(w, h) -> element or nil }.
local function card(spec, w, h)
	local role = ROLES[spec.key]
	local inner_w = w - 2 * PAD
	local name = "card/" .. spec.key
	local over = u:hovered(name)
	local open = S.open_card == spec.key
	local box = b.div():w(w):h(h):no_shrink():col():gap(GAP):p(PAD):rounded(kit.SIZE.radius):bg("raised1"):clip()
		:press(name):role("button"):label("Show the details of " .. spec.title)
		:border(1, b.alpha(role, open and 0.9 or (over and 0.5 or 0)))
		:motion({ seconds = 0.12, color = true })
	local head = b.div():row():items_center():gap(8):h(HEAD):no_shrink()
		:child(b.text(spec.title):weight(650):color(role):no_shrink())
	local used = 2 * PAD
	if h >= ROOMY then
		-- Without the room for its caption, the card says it beside its title where nothing else is.
		local side = spec.side or (h < FULL and spec.caption) or nil
		if side and w >= 200 then
			head:child(b.div():grow(1):min_w(0):row():justify_end():child(quiet(side)))
		end
		box:child(head)
		used += HEAD + GAP
		if h >= FULL and spec.caption then
			box:child(b.div():h(CAPTION):no_shrink():child(b.text(spec.caption):text_size(11.5):color("dim"):truncate():min_w(0)))
			used += CAPTION + GAP
		end
		local big = b.div():row():items_center():h(BIG):no_shrink():child(value_of(spec.number, spec.unit, spec.value_role, 28))
		if spec.badge then
			big:child(b.spacer()):child(u:badge(spec.badge[1], { tone = spec.badge[2] }))
		end
		box:child(big)
		box:child(figures(spec.figures, inner_w))
		used += BIG + FIGURES + 2 * GAP
	else
		head:child(b.spacer()):child(value_of(spec.number, spec.unit, spec.value_role, 18))
		box:child(head)
		used += HEAD + GAP + LINE + GAP
	end
	local graph_h = h - used
	local strip = spec.strip and h >= ROOMY and graph_h - STRIP - GAP >= GRAPH_MIN * 2
	if strip then
		graph_h -= STRIP + GAP
	end
	if graph_h >= GRAPH_MIN then
		box:child(spec.graph(inner_w, graph_h, role))
	else
		box:child(b.spacer())
	end
	if strip then
		box:child(spec.strip(inner_w, STRIP))
	end
	if h < ROOMY then
		box:child(b.div():row():items_center():h(LINE):no_shrink():child(figures_line(spec.figures)))
	end
	return box
end

-- The value of a share drawn in the card's own ink, and in a warning's where it is high.
local function share_role(share)
	local role = format.load_role(share)
	return role == "accent" and "foreground" or role
end

-- --- The cards ---------------------------------------------------------------------------------------

local function cpu_card()
	local sample = S.sample
	local machine = S.boot.machine
	local total = sample and sample.cpu.total or 0
	local list = {}
	if sample then
		list[1] = { "Load", string.format("%.2f", sample.load[1]) }
		local busiest = 0
		for _, core in sample.cpu.cores do
			busiest = math.max(busiest, core)
		end
		list[2] = { "Top core", format.percent(busiest) }
		local sum, count = 0, 0
		for _, freq in sample.cpu.freq do
			if freq > 0 then
				sum += freq
				count += 1
			end
		end
		if count > 0 then
			list[3] = { "Frequency", string.format("%.1f GHz", sum / count / 1000) }
		else
			list[3] = { "Threads", tostring(machine.threads) }
		end
	end
	return {
		key = "cpu",
		title = "CPU",
		side = machine.cpu,
		caption = string.format("Now, on %d threads", machine.threads),
		number = tostring(math.floor(total + 0.5)),
		unit = "%",
		value_role = share_role(total),
		figures = list,
		graph = function(w, h, role)
			return graph.dots({ { values = S.series.cpu, role = role } }, cards.window(), 100, w, h)
		end,
		strip = machine.threads > 1 and sample and function(w, h)
			return graph.bars(sample.cpu.cores, ROLES.cpu, w, h)
		end or nil,
	}
end

-- Memory pressure as a word and a tone: what the system says of how hard memory is to come by.
local function pressure(share)
	if share == nil then
		return nil
	elseif share >= 80 then
		return { "Critical", "bad" }
	elseif share >= 50 then
		return { "Elevated", "warn" }
	end
	return { "Normal", "ok" }
end

local function memory_card()
	local mem = S.sample and S.sample.mem
	local number, unit = bytes_apart(mem and mem.used or 0)
	local share = mem and mem.total > 0 and mem.used / mem.total * 100 or 0
	local list = {}
	if mem then
		local parts = mem.breakdown
		if parts ~= nil and (parts.wired > 0 or parts.compressed > 0) then
			list = { { "App", short(parts.app) }, { "Wired", short(parts.wired) }, { "Compressed", short(parts.compressed) } }
		elseif parts ~= nil then
			list = { { "App", short(parts.app) }, { "Cached", short(parts.cached) }, { "Free", short(parts.free) } }
		else
			list = { { "Used", format.percent(share) }, { "Available", short(mem.available) } }
		end
		if mem.swapTotal > 0 and #list < 3 then
			list[#list + 1] = { "Swap", short(mem.swapUsed) }
		end
	end
	return {
		key = "memory",
		title = "Memory",
		caption = mem and string.format("In use of %s", format.bytes(mem.total)) or nil,
		number = number,
		unit = unit,
		value_role = share_role(share),
		badge = mem and pressure(mem.pressure) or nil,
		figures = list,
		graph = function(w, h, role)
			return graph.dots({ { values = S.series.mem, role = role } }, cards.window(), 100, w, h)
		end,
	}
end

local function network_card()
	local net = S.sample and S.sample.net
	local number, unit = bytes_apart(net and net.rx or 0, true)
	local list = {}
	if net then
		list = {
			{ "Uploading", short_rate(net.tx) },
			{ "Received", short(net.totalRx) },
			{ "Sent", short(net.totalTx) },
		}
	end
	local busiest = net and net.interfaces[1]
	return {
		key = "network",
		title = "Network",
		side = busiest and busiest.name or nil,
		caption = "Downloading",
		number = number,
		unit = unit,
		value_role = "foreground",
		figures = list,
		graph = function(w, h, role)
			local count = cards.window()
			local drawn = { { values = S.series.rx, role = role }, { values = S.series.tx, role = SECOND } }
			return graph.dots(drawn, count, graph.ceiling(drawn, count, 10 * 1024) * 1.1, w, h)
		end,
	}
end

local function disk_card()
	local disk = S.sample and S.sample.disk
	local io = S.boot.features.diskIo
	-- The program tells the volume the person's files are on first.
	local volume = disk and disk.volumes[1]
	local free = volume and math.max(volume.total - volume.used, 0) or 0
	local number, unit = bytes_apart(free)
	local used_share = volume and volume.total > 0 and volume.used / volume.total * 100 or 0
	local list = {}
	if disk and io then
		list = { { "Reading", short_rate(disk.read) }, { "Writing", short_rate(disk.write) }, { "Used", format.percent(used_share) } }
	elseif volume then
		list = { { "Used", short(volume.used) }, { "Size", short(volume.total) }, { "Volumes", tostring(#disk.volumes) } }
	end
	local spec = {
		key = "disk",
		title = "Disk",
		side = volume and mount_name(volume.mount) or nil,
		caption = volume and string.format("Free of %s", format.bytes(volume.total)) or nil,
		number = number,
		unit = unit,
		value_role = share_role(used_share),
		figures = list,
	}
	if io then
		spec.graph = function(w, h, role)
			local count = cards.window()
			local drawn = { { values = S.series.read, role = role }, { values = S.series.write, role = SECOND_DISK } }
			return graph.dots(drawn, count, graph.ceiling(drawn, count, 100 * 1024) * 1.1, w, h)
		end
	else
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

-- The GPU card: the busiest GPU's load, and what it is made of. With several GPUs the card speaks
-- of the busiest and says how many there are.
local function gpu_card()
	local gpus = S.sample and S.sample.gpus or {}
	local busiest = gpus[1]
	for _, one in gpus do
		if one.load > busiest.load then
			busiest = one
		end
	end
	local load = busiest and busiest.load or 0
	local list = {}
	if busiest then
		if busiest.memoryUsed ~= nil then
			list[#list + 1] = { "Memory", short(busiest.memoryUsed) }
		end
		local values = S.series.gpu
		local from = math.max(1, #values - cards.window() + 1)
		local sum, peak, n = 0, 0, 0
		for index = from, #values do
			sum += values[index]
			peak = math.max(peak, values[index])
			n += 1
		end
		if n > 0 then
			list[#list + 1] = { "Average", format.percent(sum / n) }
			list[#list + 1] = { "Peak", format.percent(peak) }
		end
	end
	return {
		key = "gpu",
		title = "GPU",
		side = #gpus > 1 and string.format("%d GPUs", #gpus) or (busiest and busiest.name or nil),
		caption = #gpus > 1 and "The busiest, now" or "Now",
		number = tostring(math.floor(load + 0.5)),
		unit = "%",
		value_role = share_role(load),
		figures = list,
		graph = function(w, h, role)
			return graph.dots({ { values = S.series.gpu, role = role } }, cards.window(), 100, w, h)
		end,
	}
end

-- --- The gauges -------------------------------------------------------------------------------------------

local GAUGE = 20
local NAME_W = 64

-- The cards as a line each, for a pane too narrow for a graph each: a name, a meter and the
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
	local busiest = nil
	for _, one in sample.gpus do
		if busiest == nil or one.load > busiest.load then
			busiest = one
		end
	end
	if busiest then
		line("GPU", busiest.load, string.format("%s  %s", format.percent(busiest.load), #sample.gpus > 1 and string.format("%d GPUs", #sample.gpus) or ""), "gpu")
	end
	return box
end

-- How many cards the overview has: four, and the GPU's where the machine has one.
function cards.count()
	return S.boot and S.boot.features.gpu and 5 or 4
end

-- The cards at `w` by `h` points each, in the order of the overview.
function cards.list(w, h)
	local list = {
		card(cpu_card(), w, h),
		card(memory_card(), w, h),
		card(network_card(), w, h),
		card(disk_card(), w, h),
	}
	if cards.count() == 5 then
		list[#list + 1] = card(gpu_card(), w, h)
	end
	return list
end

return cards
