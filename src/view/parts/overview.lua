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

-- The band over the table: four cards in a row on a wide page, four gauges on a narrow one. The
-- table is what the page is for; the band is what is left of the machine around it.
local CARD_H = 124
-- A page this tall has room for the cards with their detail lines.
local ROOMY_CARD_H = 196
local ROOMY_PAGE = 900
local WIDE_FROM = 760

function overview.view()
	if S.width >= WIDE_FROM then
		local w = math.floor((S.width - 2 * PAGE_PAD - 3 * GAP) / 4)
		local list = cards.list(w, S.height >= ROOMY_PAGE and ROOMY_CARD_H or CARD_H)
		local row = b.div():row():gap(GAP):px(PAGE_PAD):pt(PAGE_PAD):no_shrink()
		for _, one in list do
			row:child(one)
		end
		return row
	end
	return b.div():px(PAGE_PAD):pt(PAGE_PAD / 2):no_shrink():child(cards.gauges(S.width - 2 * PAGE_PAD))
end

return overview
