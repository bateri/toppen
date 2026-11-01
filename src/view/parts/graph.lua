-- The graphs of the cards: areas under a line, drawn on a canvas, the newest value at the right.
-- A canvas has no size of its own and draws in its own points, so each graph is given the width and
-- height it is laid out at.

local kit = require("../lib/kit")

local b = kit.b

local graph = {}

-- The newest `count` values of `values`, with how many there are.
local function tail(values, count)
	local n = #values
	local from = math.max(1, n - count + 1)
	local out = {}
	for index = from, n do
		out[#out + 1] = values[index]
	end
	return out
end

-- The biggest of the values of every series in `lines` over the last `count` samples, the least a
-- ceiling may be being `floor`: a graph of a quiet network is not stretched to look busy.
function graph.ceiling(lines, count, floor)
	local top = floor
	for _, line in lines do
		for _, value in tail(line.values, count) do
			if value > top then
				top = value
			end
		end
	end
	return top
end

-- An area graph of `lines` (each { values =, role = }), the last `count` samples of them, between 0
-- and `ceiling`, `w` by `h` points. A line shorter than `count` starts from the right. `back` (a
-- whole number of samples from the newest, or nil) marks one moment: a rule down the graph and a dot
-- on each line, where the pointer is.
function graph.area(lines, count, ceiling, w, h, back)
	local paths = {}
	-- Rules at the half and the top, quiet.
	for _, fraction in { 0.5 } do
		local y = math.floor(h * fraction) + 0.5
		paths[#paths + 1] = b.path():move_to(0, y):line_to(w, y):stroke(1, b.alpha("foreground", 0.07))
	end
	local step = count > 1 and w / (count - 1) or w
	local inner = h - 2
	for _, line in lines do
		local values = tail(line.values, count)
		local n = #values
		if n >= 2 then
			local x0 = w - (n - 1) * step
			local outline = b.path()
			local area = b.path()
			for index, value in values do
				local x = x0 + (index - 1) * step
				local y = 1 + inner - math.min(math.max(value / ceiling, 0), 1) * inner
				if index == 1 then
					area:move_to(x, h):line_to(x, y)
					outline:move_to(x, y)
				else
					area:line_to(x, y)
					outline:line_to(x, y)
				end
			end
			area:line_to(w, h):close():fill(b.alpha(line.role, 0.2))
			outline:stroke(1.5, line.role)
			paths[#paths + 1] = area
			paths[#paths + 1] = outline
		end
	end
	if back ~= nil and back >= 0 and back <= count - 1 then
		local x = w - back * step
		paths[#paths + 1] = b.path():move_to(x, 0):line_to(x, h):stroke(1, b.alpha("foreground", 0.45))
		for _, line in lines do
			local values = tail(line.values, count)
			local value = values[#values - back]
			if value ~= nil then
				local y = 1 + inner - math.min(math.max(value / ceiling, 0), 1) * inner
				paths[#paths + 1] = b.path():rect(x - 2, y - 2, 4, 4):fill(line.role)
			end
		end
	end
	return b.div():w(w):h(h):no_shrink():clip():child(b.canvas(paths):size(w, h))
end

-- A dot matrix of `lines` (each { values =, role = }, one or two), the last `count` samples of
-- them between 0 and `ceiling`, `w` by `h` points, on a ground of the first line's colour: a column
-- of dots for each moment, the newest at the right, lit from the bottom as high as the value. Where
-- two lines are drawn the column is lit to the higher of them, the part both reach in the colour of
-- the lower one. The dots are drawn as few paths, one for each colour, cut where a path would grow
-- past what the kit takes.
local PITCH = 5
local DOT = 1.2
-- The biggest a dot is, and the most columns and rows there are.
local MOST_DOT = 2.2
local MOST_COLUMNS = 90
local MOST_ROWS = 28
-- Dots a path holds at most: two steps each, under the kit's 4096.
local PER_PATH = 1800
-- How long the line of a dot is.
local JOT = 0.01

-- The dots each graph was last drawn with, by where it is (its colours and its size), and what
-- they were drawn of: the page draws far more often than a sample comes (the pointer, a drag of an
-- edge), and the same samples make the same dots. Only what the last frame drew is kept, the last
-- drawing of each place: a drawing is thousands of dots, and a few dozen of them are more than the
-- page's memory.
local drawn_before = {}
local drawn_now = {}

-- A frame begins: what the frame before did not draw is forgotten.
function graph.begin()
	drawn_before, drawn_now = drawn_now, {}
end

function graph.dots(lines, count, ceiling, w, h)
	local inset = 6
	local role = lines[1].role
	local box = b.div():w(w):h(h):no_shrink():rounded(6):bg(b.alpha(role, 0.08)):clip()
	local place = table.concat({ w, h, role }, ":")
	local key = table.concat({ count, ceiling }, ":")
	for _, line in lines do
		place ..= ":" .. tostring(line.role)
		key ..= ":" .. #line.values .. ":" .. tostring(line.values[#line.values])
	end
	local before = drawn_now[place] or drawn_before[place]
	if before ~= nil and before.key == key then
		drawn_now[place] = before
		return box:child(b.canvas(before.paths):size(w, h))
	end
	-- What this place was drawn with goes now, not at the next frame.
	drawn_before[place] = nil
	local inner_w, inner_h = w - 2 * inset, h - 2 * inset
	-- A column for each sample at most: a wide card spaces its dots out and makes them bigger, as
	-- many as a narrow one has, not more (each is a part of a path the page holds).
	local cols = math.max(math.min(math.floor((inner_w + PITCH - 2 * DOT) / PITCH), count, MOST_COLUMNS), 1)
	local pitch = cols > 1 and math.max(PITCH, (inner_w - 2 * DOT) / (cols - 1)) or PITCH
	local dot = math.min(DOT * pitch / PITCH, MOST_DOT)
	if cols > 1 and dot > DOT then
		-- The bigger dots stand as far in from the edge as the small ones.
		pitch = math.max(PITCH, (inner_w - 2 * dot) / (cols - 1))
	end
	local rows = math.max(math.min(math.floor((inner_h - 2 * dot) / pitch) + 1, MOST_ROWS), 1)
	-- Each column is the highest of the samples that fall in it.
	local series = {}
	for index, line in lines do
		local values = tail(line.values, count)
		local lit = {}
		for column = 1, cols do
			local from = math.floor((column - 1) * count / cols) + 1 - (count - #values)
			local to = math.floor(column * count / cols) - (count - #values)
			local top = nil
			for at = math.max(from, 1), math.max(to, from) do
				local value = values[at]
				if value ~= nil and (top == nil or value > top) then
					top = value
				end
			end
			if top == nil then
				lit[column] = -1
			else
				local share = math.min(math.max(top / math.max(ceiling, 1e-9), 0), 1)
				lit[column] = share > 0 and math.max(math.floor(share * rows + 0.5), 1) or 0
			end
		end
		series[index] = lit
	end
	local groups = {}
	local function put(color, x, y)
		local group = groups[color]
		if group == nil then
			group = { paths = {}, count = 0 }
			groups[color] = group
		end
		if group.count % PER_PATH == 0 then
			group.paths[#group.paths + 1] = b.path()
		end
		group.count += 1
		-- A dot is a line too short to see, stroked: the kit's lines end round, so it is a
		-- circle of the stroke's width in two steps, where `circle` takes six.
		local segments = group.paths[#group.paths].segments
		local n = #segments
		segments[n + 1] = { "move", x, y }
		segments[n + 2] = { "line", x, y + JOT }
	end
	local quiet = b.alpha(role, 0.16)
	local x0 = w - inset - dot - (cols - 1) * pitch
	local y0 = h - inset - dot
	for column = 1, cols do
		local first = series[1][column]
		local second = series[2] and series[2][column] or -1
		local high = math.max(first, second)
		local low_role = role
		if series[2] ~= nil and second >= 0 and second < first then
			low_role = lines[2].role
		elseif series[2] ~= nil and second > first then
			low_role = role
		end
		local both = series[2] ~= nil and math.min(first, second) or high
		local high_role = (series[2] ~= nil and second > first) and lines[2].role or role
		local x = x0 + (column - 1) * pitch
		for row = 1, rows do
			local y = y0 - (row - 1) * pitch
			if row <= both then
				put(low_role, x, y)
			elseif row <= high then
				put(high_role, x, y)
			else
				put(quiet, x, y)
			end
		end
	end
	local paths = {}
	for color, group in groups do
		for _, path in group.paths do
			paths[#paths + 1] = path:stroke(2 * dot, color)
		end
	end
	-- Kept for the next frame, in place of what this place was drawn with before.
	drawn_now[place] = { key = key, paths = paths }
	return box:child(b.canvas(paths):size(w, h))
end

-- A row of thin bars, one for each value (0 to 100): the processors at this moment.
function graph.bars(values, role, w, h)
	local paths = {}
	local n = #values
	if n == 0 then
		return b.div():w(w):h(h):no_shrink()
	end
	local gap = n > 1 and math.min(2, math.max(w / n * 0.2, 0.5)) or 0
	local width = (w - gap * (n - 1)) / n
	for index, value in values do
		local bar = math.max(math.min(value, 100) / 100 * h, 1)
		local x = (index - 1) * (width + gap)
		paths[#paths + 1] = b.path():rect(x, 0, width, h):fill(b.alpha("foreground", 0.07))
		paths[#paths + 1] = b.path():rect(x, h - bar, width, bar):fill(value >= 90 and "error" or (value >= 70 and "warning" or role))
	end
	return b.div():w(w):h(h):no_shrink():clip():child(b.canvas(paths):size(w, h))
end

-- A bar `fraction` full (0 to 1), `w` by `h` points.
function graph.meter(fraction, role, w, h)
	fraction = math.max(0, math.min(1, fraction or 0))
	local track = b.div():w(w):h(h):rounded(h / 2):bg(b.alpha("foreground", 0.08)):clip():no_shrink()
	if fraction > 0 then
		track:child(b.div():w(math.max(w * fraction, h)):h(h):rounded(h / 2):bg(role))
	end
	return track
end

return graph
