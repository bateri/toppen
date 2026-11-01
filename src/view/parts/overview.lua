-- The overview: what the program says of the machine (`overview`), kept as the series the cards
-- draw, and the cards laid out over the page.

local kit = require("../lib/kit")
local cards = require("./cards")

local b, S, SIZE = kit.b, kit.S, kit.SIZE

local overview = {}

local PAGE_PAD = SIZE.pad
local GAP = SIZE.gap

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
	series.gpu = told.gpu
	series.at = told.at
	-- Each logical processor's own series, from the columns of samples.
	S.cores = {}
	for _, row in told.cores do
		for index, value in row do
			local list = S.cores[index]
			if list == nil then
				list = {}
				S.cores[index] = list
			end
			list[#list + 1] = value
		end
	end
	-- And each GPU's, the same way.
	S.gpus = {}
	for _, row in told.gpus do
		for index, value in row do
			local list = S.gpus[index]
			if list == nil then
				list = {}
				S.gpus[index] = list
			end
			list[#list + 1] = value
		end
	end
	while #series.cpu > SIZE.kept do
		for _, list in series do
			table.remove(list, 1)
		end
		for _, list in S.cores do
			table.remove(list, 1)
		end
		for _, list in S.gpus do
			table.remove(list, 1)
		end
	end
end

-- The series of each interface the machine has carried something on: what it did at each sample,
-- nothing where it was quiet.
local function interfaces(told)
	local seen = {}
	for _, one in told.net.interfaces do
		seen[one.name] = one
		if S.ifaces[one.name] == nil and S.iface_count < 24 then
			S.iface_count += 1
			S.ifaces[one.name] = { rx = {}, tx = {} }
		end
	end
	for name, lists in S.ifaces do
		local one = seen[name]
		push(lists.rx, one and one.rx or 0)
		push(lists.tx, one and one.tx or 0)
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
	push(series.at, told.at)
	local busiest = 0
	for index, one in told.gpus do
		busiest = math.max(busiest, one.load)
		local list = S.gpus[index]
		if list == nil then
			list = {}
			S.gpus[index] = list
		end
		push(list, one.load)
	end
	push(series.gpu, busiest)
	for index, value in told.cpu.cores do
		local list = S.cores[index]
		if list == nil then
			list = {}
			S.cores[index] = list
		end
		push(list, value)
	end
	interfaces(told)
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

-- The band over the table: the cards in a row on a wide page (four, and a fifth where the machine
-- has a GPU), the same as gauges on a narrow one. The table is what the page is for; the band is
-- what is left of the machine around it.
local CARD_H = 124
-- A page this tall has room for the cards with their figures on a line of their own; a taller one
-- for their captions too, and a taller one again for a deeper graph.
local MIDDLE_CARD_H = 164
local MIDDLE_PAGE = 600
local FULL_CARD_H = 214
local FULL_PAGE = 760
local ROOMY_CARD_H = 240
local ROOMY_PAGE = 960
local CARD_W = 190

function overview.view()
	local count = cards.count()
	if S.width >= CARD_W * count then
		local w = math.floor((S.width - 2 * PAGE_PAD - (count - 1) * GAP) / count)
		local h = CARD_H
		if S.height >= ROOMY_PAGE then
			h = ROOMY_CARD_H
		elseif S.height >= FULL_PAGE then
			h = FULL_CARD_H
		elseif S.height >= MIDDLE_PAGE then
			h = MIDDLE_CARD_H
		end
		local list = cards.list(w, h)
		local row = b.div():row():gap(GAP):px(PAGE_PAD):pt(PAGE_PAD):no_shrink()
		for _, one in list do
			row:child(one)
		end
		return row
	end
	return b.div():px(PAGE_PAD):pt(PAGE_PAD / 2):no_shrink():child(cards.gauges(S.width - 2 * PAGE_PAD))
end

return overview
