-- The status line (the library's `u:status`): the machine, how long it has been up, how often it is
-- looked at, and the one thing to do about it: pause.

local kit = require("../lib/kit")
local format = require("../lib/format")
local commands = require("./commands")

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
			-- What the person chose is what the next run starts with (the settings file, where it can be).
			if params.interval ~= nil and act.save_setting ~= nil then
				act.save_setting({ interval = result.interval }, true)
			end
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

-- An action of the line: the item that was pressed. Whether it was one of this line's.
function status.action(action)
	if action.from ~= "status" or action.kind ~= "press" then
		return false
	end
	if action.key == "pause" then
		act.toggle_pause()
	elseif action.key == "help" then
		act.open_help()
	elseif action.key == "interval" then
		-- Round the steps: slower, and from the slowest back to the fastest.
		if S.interval >= STEPS[#STEPS] - 0.01 then
			set({ interval = STEPS[1] })
		else
			act.step_interval(1)
		end
	end
	return true
end

function status.view()
	local machine = S.boot.machine
	local start = { { key = "host", words = kit.host_shown(machine.host), priority = 9 } }
	if S.layout ~= "narrow" then
		start[#start + 1] = { key = "os", words = machine.os, priority = 3 }
	end
	if S.sample then
		start[#start + 1] = { key = "up", words = "up " .. format.duration(S.sample.uptime), priority = 6 }
		if S.layout == "wide" then
			local load = S.sample.load
			start[#start + 1] = { key = "load", words = string.format("load %.2f %.2f %.2f", load[1], load[2], load[3]), priority = 4 }
		end
	end
	local finish = {}
	if S.paused then
		finish[#finish + 1] = { key = "paused", words = "Paused", tone = "warn", priority = 10 }
	end
	finish[#finish + 1] = {
		key = "interval",
		words = "every " .. format.interval(S.interval),
		press = true,
		tip = "Update interval · click to change · + and − step it",
		priority = 5,
	}
	finish[#finish + 1] = {
		key = "help",
		words = "Keys",
		press = true,
		tip = "Show the keys and the commands",
		keys = commands.first("help"),
		priority = 6,
	}
	finish[#finish + 1] = {
		key = "pause",
		words = S.paused and "Resume" or "Pause",
		press = true,
		tip = S.paused and "Resume updates" or "Pause updates",
		keys = commands.first("pause"),
		tone = S.paused and "warn" or nil,
		priority = 8,
	}
	return kit.u:status("status", { start = start, finish = finish })
end

return status
