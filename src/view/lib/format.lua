-- Numbers as a person reads them. The program says bytes, percent (0 to 100) and seconds; this is the
-- one place they become words, so that the cards, the status line and the tips say them alike.

local format = {}

local UNITS = { "B", "KiB", "MiB", "GiB", "TiB" }

-- Bytes: 1024 to the step, one decimal.
function format.bytes(count)
	count = math.max(count or 0, 0)
	local unit = 1
	while count >= 1024 and unit < #UNITS do
		count = count / 1024
		unit += 1
	end
	if unit == 1 then
		return string.format("%d B", count)
	end
	return string.format("%.1f %s", count, UNITS[unit])
end

-- The same in three characters of number at most, for a big value: 12 MiB, 1.5 GiB, 340 KiB.
function format.short_bytes(count)
	count = math.max(count or 0, 0)
	local unit = 1
	while count >= 1024 and unit < #UNITS do
		count = count / 1024
		unit += 1
	end
	if unit == 1 then
		return string.format("%d B", count)
	elseif count >= 100 then
		return string.format("%d %s", count, UNITS[unit])
	elseif count >= 10 then
		return string.format("%.0f %s", count, UNITS[unit])
	end
	return string.format("%.1f %s", count, UNITS[unit])
end

function format.rate(per_second)
	return format.short_bytes(per_second) .. "/s"
end

-- A share, 0 to 100, as a whole percent.
function format.percent(share)
	return string.format("%d%%", math.floor((share or 0) + 0.5))
end

-- "3d 4h", "2h 5m", "7m", "40s": the two biggest steps.
function format.duration(seconds)
	seconds = math.max(math.floor(seconds or 0), 0)
	local days = seconds // 86400
	local hours = (seconds % 86400) // 3600
	local minutes = (seconds % 3600) // 60
	if days > 0 then
		return string.format("%dd %dh", days, hours)
	elseif hours > 0 then
		return string.format("%dh %dm", hours, minutes)
	elseif minutes > 0 then
		return string.format("%dm", minutes)
	end
	return string.format("%ds", seconds)
end

-- How long a stretch of a graph is: "60 s", "2 min", "2 min 30 s".
function format.span(seconds)
	seconds = math.floor(seconds + 0.5)
	if seconds < 90 then
		return string.format("%d s", seconds)
	end
	local minutes, rest = seconds // 60, seconds % 60
	if rest == 0 then
		return string.format("%d min", minutes)
	end
	return string.format("%d min %d s", minutes, rest)
end

-- Seconds between two updates: "1 s", "0.5 s".
function format.interval(seconds)
	if seconds == math.floor(seconds) then
		return string.format("%d s", seconds)
	end
	return string.format("%.1f s", seconds)
end

-- The role a share of something used is drawn in: calm, then a warning, then trouble.
function format.load_role(share)
	if share >= 90 then
		return "error"
	elseif share >= 70 then
		return "warning"
	end
	return "accent"
end

return format
