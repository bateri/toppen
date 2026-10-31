-- The overview: what the program says of the machine (`overview`), kept as the series the cards
-- draw, and the cards laid out over the page.

local kit = require("../lib/kit")
local cards = require("./cards")

local b, S, SIZE = kit.b, kit.S, kit.SIZE

local overview = {}

local PAGE_PAD = 10
local GAP = 8

-- Adds `value` to the end of `list`, forgetting the oldest past what the page keeps.
local function push(list, value)
	list[#list + 1] = value
	if #list > SIZE.kept then
		table.remove(list, 1)
	end
end

-- The history the program holds, as columns, in place of the series.
local function history(told)
	local series = S.series
	series.cpu, series.mem, series.swap = told.cpu, told.mem, told.swap
	series.rx, series.tx = told.rx, told.tx
	series.read, series.write = told.read, told.write
	while #series.cpu > SIZE.kept do
		for _, list in series do
			table.remove(list, 1)
		end
	end
end

-- One sample: the latest, and one more point of each series.
local function sampled(told)
	local series = S.series
	S.sample = told
	push(series.cpu, told.cpu.total)
	push(series.mem, told.mem.total > 0 and told.mem.used / told.mem.total * 100 or 0)
	push(series.swap, told.mem.swapTotal > 0 and told.mem.swapUsed / told.mem.swapTotal * 100 or 0)
	push(series.rx, told.net.rx)
	push(series.tx, told.net.tx)
	push(series.read, told.disk.read)
	push(series.write, told.disk.write)
end

-- Listens to the `overview` topic.
function overview.start()
	b.subscribe("overview", {}, function(told)
		if told.kind == "history" then
			history(told)
		elseif told.kind == "sample" then
			sampled(told)
		end
		return kit.take(true)
	end)
end

-- Four cards in a row on a wide, short page; two and two otherwise.
function overview.view()
	local rows = (S.width >= 900 and S.height < 460) and 1 or 2
	local columns = 4 // rows
	local height = S.height - SIZE.status - 2 * PAGE_PAD
	local w = math.floor((S.width - 2 * PAGE_PAD - (columns - 1) * GAP) / columns)
	local h = math.floor((height - (rows - 1) * GAP) / rows)
	local list = cards.list(w, h)
	local grid = b.div():col():gap(GAP):p(PAGE_PAD):grow(1):min_h(0)
	for row = 1, rows do
		local line = b.div():row():gap(GAP):no_shrink()
		for column = 1, columns do
			line:child(list[(row - 1) * columns + column])
		end
		grid:child(line)
	end
	return grid
end

return overview
