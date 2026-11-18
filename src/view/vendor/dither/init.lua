-- A dithered look for the page's charts, after dither-kit-rs's
-- (https://github.com/DeepanshuMishraa/dither-kit-rs), written as a kit from outside pencere would
-- be: on the page API alone, copied into the page's own files. A chart is drawn as a grid of square
-- dots, each lit where a level (0 to 1) is above the ordered (Bayer) threshold of its place, so that
-- a fade is a pattern of whole dots and the same values are the same dots on every frame.
--
-- `dither.area` and `dither.marker` are a painter: they take what an area chart is (the series'
-- points, the rules, the moment under the pointer) and give the paths of a canvas, as the chart's
-- own smooth painter does.

local dither = {}

-- The side of a dot, points. A path is drawn crisp where the terminal's kit can (`crisp`), so that
-- a dot is whole pixels at any scale; an older kit smooths its edges.
local DOT = 2
local CRISP = pencere.Path ~= nil and pencere.Path.crisp ~= nil
-- Dots a path holds at most: a rectangle is five steps, a path takes 4096. Each run of lit dots is a
-- rectangle, and each step of it a table of the page's memory: a page draws a few charts in dots, not
-- dozens.
local PER_PATH = 800

-- The Bayer matrix of side `size` (a power of two), as thresholds between 0 and 1.
local function bayer(size)
	local m, n = { { 0 } }, 1
	while n < size do
		local out = {}
		for y = 1, 2 * n do
			out[y] = {}
		end
		for y = 1, n do
			for x = 1, n do
				local v = 4 * m[y][x]
				out[y][x] = v
				out[y][x + n] = v + 2
				out[y + n][x] = v + 3
				out[y + n][x + n] = v + 1
			end
		end
		m, n = out, 2 * n
	end
	for y = 1, n do
		for x = 1, n do
			m[y][x] = (m[y][x] + 0.5) / (n * n)
		end
	end
	return m, n
end

local MATRIX, SIDE = bayer(8)

-- The threshold of the dot at `col`, `row` (from 0).
local function threshold(col, row)
	return MATRIX[row % SIDE + 1][col % SIDE + 1]
end

dither.threshold = threshold

-- The paths of a grid of dots `cols` by `rows`, `dot` points each, from `(x0, y0)`: `color_at(col,
-- row)` says the colour a dot is lit in, or nil. The lit dots of a row that touch, in one colour,
-- are one rectangle.
function dither.grid(cols, rows, color_at, dot, x0, y0)
	dot = dot or DOT
	x0, y0 = x0 or 0, y0 or 0
	local groups, order = {}, {}
	local function rect(color, from, to, row)
		local group = groups[color]
		if group == nil then
			group = { paths = {}, count = 0 }
			groups[color] = group
			order[#order + 1] = color
		end
		if group.count % PER_PATH == 0 then
			group.paths[#group.paths + 1] = pencere.path()
		end
		group.count += 1
		group.paths[#group.paths]:rect(x0 + from * dot, y0 + row * dot, (to - from) * dot, dot)
	end
	for row = 0, rows - 1 do
		local run, start = nil, 0
		for col = 0, cols do
			local color = col < cols and color_at(col, row) or nil
			if color ~= run then
				if run ~= nil then
					rect(run, start, col, row)
				end
				run, start = color, col
			end
		end
	end
	local paths = {}
	for _, color in order do
		for _, path in groups[color].paths do
			path:fill(color)
			if CRISP then
				path:crisp()
			end
			paths[#paths + 1] = path
		end
	end
	return paths
end

-- The height of `points` (a list of { x, y }, x rising) at `x`, or nil outside them.
local function height_at(points, x)
	local n = #points
	if n < 2 or x < points[1][1] or x > points[n][1] then
		return nil
	end
	local lo, hi = 1, n
	while hi - lo > 1 do
		local mid = (lo + hi) // 2
		if points[mid][1] <= x then
			lo = mid
		else
			hi = mid
		end
	end
	local a, b = points[lo], points[hi]
	local span = b[1] - a[1]
	if span <= 0 then
		return math.min(a[2], b[2])
	end
	return a[2] + (b[2] - a[2]) * (x - a[1]) / span
end

-- How lit the area under a line is, `below` dots under it: dense at the line and fading over a
-- fixed depth to a quiet floor, so that a low line has an area that reads as one, and a high one a
-- foot that is not empty.
local FADE_DOTS = 16
local function fill_level(below)
	local near = math.max(1 - below / FADE_DOTS, 0)
	return 0.12 + 0.5 * near ^ 1.5
end

-- The painter of an area chart: `area(model)` draws its series and rules, `marker(model)` the
-- moment under the pointer, so that a page can keep the first while the pointer moves. `model`:
--
--   { w =, h =,                                           -- the box, points
--     rules = { y, … },                                   -- quiet rules across it
--     series = { { role =, points = { { x, y }, … } } },  -- each line, x rising, in the box's points
--     marker = { x =, points = { { x, y, role }, … } } }  -- the moment under the pointer, or nil
--
-- Later series are drawn over earlier ones, and a line over every area.
function dither.area(model)
	local dot = DOT
	local cols, rows = math.floor(model.w / dot), math.floor(model.h / dot)
	-- Each series' line, as a row of dots, at the middle of each column.
	local lines = {}
	for index, series in model.series do
		local at = {}
		for col = 0, cols - 1 do
			local y = height_at(series.points, (col + 0.5) * dot)
			at[col] = y and y / dot or nil
		end
		lines[index] = at
	end
	local rule_rows = {}
	for _, y in model.rules or {} do
		rule_rows[math.floor(y / dot)] = true
	end
	local quiet = pencere.alpha("foreground", 0.22)
	return dither.grid(cols, rows, function(col, row)
		local center = row + 0.5
		for index = #model.series, 1, -1 do
			local y = lines[index][col]
			if y ~= nil and math.abs(center - y) <= 0.75 then
				return model.series[index].role
			end
		end
		local lit = nil
		for index, series in model.series do
			local y = lines[index][col]
			if y ~= nil and center > y then
				if fill_level(center - y) > threshold(col, row) then
					lit = series.role
				end
			end
		end
		if lit == nil and rule_rows[row] and col % 2 == 0 then
			return quiet
		end
		return lit
	end, dot)
end

-- A column of every other dot down the chart where the pointer is, and on each line there a block of
-- three by three dots in the foreground with the line's colour at its middle, so that it stands out
-- of a line of its own colour.
function dither.marker(model)
	local dot = DOT
	local marker = model.marker
	local rows = math.floor(model.h / dot)
	local x = math.floor(marker.x / dot) * dot
	local column = pencere.path()
	for row = 0, rows - 1, 2 do
		column:rect(x, row * dot, dot, dot)
	end
	local paths = { column:fill(pencere.alpha("foreground", 0.5)) }
	for _, point in marker.points do
		local row = math.floor(point[2] / dot)
		paths[#paths + 1] = pencere.path():rect(x - dot, (row - 1) * dot, 3 * dot, 3 * dot):fill("foreground")
		paths[#paths + 1] = pencere.path():rect(x, row * dot, dot, dot):fill(point[3])
	end
	if CRISP then
		for _, path in paths do
			path:crisp()
		end
	end
	return paths
end

return dither
