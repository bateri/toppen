-- The status line: the machine, how long it has been up, how often it is looked at, and the one
-- thing to do about it: pause.

local kit = require("../lib/kit")
local format = require("../lib/format")

local b, S, SIZE, act = kit.b, kit.S, kit.SIZE, kit.act

local status = {}

-- The intervals the update rate steps through, seconds.
local STEPS = { 0.5, 1, 2, 5, 10 }

local function apply(sampling)
	S.interval, S.paused = sampling.interval, sampling.paused
end

-- Tells the program how often to look and whether to, and takes what it says it did.
local function set(params)
	kit.call("sampling", params, function(result, err)
		if result then
			apply(result)
		end
		return true
	end)
end

function act.toggle_pause()
	set({ paused = not S.paused })
end

-- A step toward faster (`-1`) or slower (`1`) updates.
function act.step_interval(direction)
	local at = 1
	for index, seconds in STEPS do
		if math.abs(seconds - S.interval) < 0.01 then
			at = index
		end
	end
	local seconds = STEPS[math.max(1, math.min(#STEPS, at + direction))]
	if seconds ~= S.interval then
		set({ interval = seconds })
	end
end

function status.start(sampling)
	apply(sampling)
end

-- An item of the line that can be pressed: its words, with the pointer's hover under it.
local function item(id, words, tip, tone)
	local over = kit.hovered(id)
	local element = b.div():press(id):row():items_center():gap(4):h(SIZE.status - 6):px(8):rounded(4):no_shrink()
		:bg(over and b.alpha("foreground", 0.1) or nil)
		:motion({ seconds = 0.11, color = true })
		:role("button")
		:label(tip or words)
		:child(b.text(words):color(tone or (over and "foreground" or "dim")))
	if tip then
		element:tip(tip)
	end
	return element
end

function status.press(id)
	if id == "status-pause" then
		act.toggle_pause()
	elseif id == "status-interval" then
		-- Round the steps: slower, and from the slowest back to the fastest.
		if S.interval >= STEPS[#STEPS] - 0.01 then
			set({ interval = STEPS[1] })
		else
			act.step_interval(1)
		end
	else
		return false
	end
	return true
end

function status.view()
	local machine = S.boot.machine
	local row = b.div():row():items_center():gap(2):h(SIZE.status):px(6):no_shrink():bg("raised1"):color("dim"):text_size(11.5)
	local function part(words, tone, shrink)
		local text = b.text(words):color(tone)
		if shrink then
			text:truncate():min_w(0)
		end
		local box = b.div():px(4):child(text):min_w(0)
		if not shrink then
			box:no_shrink()
		end
		return box
	end
	local first = true
	local function add(words, tone, shrink)
		if not first then
			row:child(b.text("·"):color("quiet"):no_shrink())
		end
		first = false
		row:child(part(words, tone, shrink))
	end
	add(machine.host, "foreground", true)
	if S.layout ~= "narrow" then
		add(machine.os, "dim", true)
	end
	if S.sample then
		add("up " .. format.duration(S.sample.uptime), "dim", false)
		if S.layout == "wide" then
			local load = S.sample.load
			add(string.format("load %.2f %.2f %.2f", load[1], load[2], load[3]), "dim", false)
		end
	end
	row:child(b.spacer())
	if S.paused then
		row:child(b.div():px(4):no_shrink():child(b.text("Paused"):color("warning"):weight(600)))
	end
	row:child(item("status-interval", "every " .. format.interval(S.interval), "Update interval · click to change · + and − step it"))
	row:child(item("status-pause", S.paused and "Resume" or "Pause", S.paused and "Resume updates · Space" or "Pause updates · Space", S.paused and "warning" or nil))
	return row
end

return status
