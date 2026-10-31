-- The page of toppen: a system monitor, drawn by the terminal in its own font and theme.
--
-- The program looks at the machine, keeps what it saw and tells the page: the cards draw the CPU,
-- memory, network and disk of this moment and of the last minute; around them the status line. The
-- page talks to the program only by the calls and topics of docs/calls.md.

local kit = require("./lib/kit")
local overview = require("./parts/overview")
local status = require("./parts/status")

local b, ui, S, SIZE, act = kit.b, kit.ui, kit.S, kit.SIZE, kit.act
local layout = b.layout

-- --- The page -----------------------------------------------------------------------------------------

local function layout_of(width)
	if width < SIZE.narrow then
		return "narrow"
	elseif width < SIZE.medium then
		return "medium"
	end
	return "wide"
end

local function booting()
	local box = b.div():size_full():col():items_center():justify_center():gap(8):bg("background"):family("terminal")
	if S.boot_error then
		box:child(b.text("The program did not answer"):weight(650))
		box:child(b.text(S.boot_error):color("dim"):wrap():max_w(520):align("center"))
	else
		box:child(kit.spinner(20, "quiet"))
	end
	return box
end

local function render(size, now)
	S.now, S.width, S.height = now, size.w, size.h
	S.layout = layout_of(size.w)
	S.short = size.h < SIZE.short
	if not S.boot then
		return booting(), false, nil
	end
	local root = b.div():size_full():col():bg("background"):color("foreground"):family("terminal")
	root:command("pause", "Pause or resume updates", "space")
	root:child(overview.view()):child(status.view())
	return root, false, nil
end

-- --- What the person does -------------------------------------------------------------------------------

local press_event = nil

local function pressed(event)
	ui:press(layout, { x = event.x, y = event.y })
	press_event = event
	return true
end

local function released(event)
	local clicked = ui:release(layout, { x = event.x, y = event.y })
	if clicked and press_event and (press_event.button or "left") == "left" then
		status.press(clicked)
	end
	return true
end

-- The pointer's shape where it is. Every reply to the pointer says it: one that says none is the
-- arrow.
local function cursor_at()
	return ui.hover and "pointer" or "arrow"
end

local function moved(event)
	local changed = ui:point(layout, { x = event.x, y = event.y })
	return { redraw = changed, cursor = cursor_at() }
end

local function key(event)
	local name = event.key
	if name == "+" or name == "=" then
		act.step_interval(-1)
		return true
	elseif name == "-" then
		act.step_interval(1)
		return true
	end
	return nil
end

local function handle(event)
	local kind = event.type
	local reply = nil
	if kind == "move" then
		reply = moved(event)
	elseif kind == "leave" then
		reply = ui:point(layout, nil)
	elseif kind == "press" then
		reply = b.merge(pressed(event), { cursor = cursor_at() })
	elseif kind == "release" then
		reply = b.merge(released(event), { cursor = cursor_at() })
	elseif kind == "key" then
		reply = key(event)
	elseif kind == "command" then
		if event.id == "pause" then
			act.toggle_pause()
		end
		reply = true
	end
	return kit.take(reply)
end

-- --- Starting --------------------------------------------------------------------------------------------

kit.call("boot", {}, function(boot, err)
	if err then
		S.boot_error = err.message or err.code
		return true
	end
	S.boot = boot
	status.start(boot.sampling)
	overview.start()
	return true
end)

return { render = render, handle = handle }
