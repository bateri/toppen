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
