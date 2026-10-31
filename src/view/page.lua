-- The page of toppen: a system monitor, drawn by the terminal in its own font and theme.
--
-- The program looks at the machine, keeps what it saw and tells the page: the cards (or, on a narrow
-- pane, the gauges) draw the CPU, memory, network and disk of this moment; under them the process
-- table, with the bar that filters it; around them the status line. The page talks to the program
-- only by the calls and topics of docs/calls.md, and draws with the kit's library (`@pencere/ui`).

local kit = require("./lib/kit")
local overview = require("./parts/overview")
local status = require("./parts/status")
local tab = require("./parts/table")
local filter = require("./parts/filter")

local b, u, S, SIZE, act, lib = kit.b, kit.u, kit.S, kit.SIZE, kit.act, kit.lib

-- --- The commands --------------------------------------------------------------------------------------------
--
-- What can be done, once: the keys, the host's list, and (later) the palette and the help are made of
-- it. A key with a letter is the page's only while the keyboard is not in a field.

local function idle()
	return u.holder == "page"
end

local cmds = lib.commands({
	{ id = "pause", title = "Pause or resume updates", group = "Updates", keys = "space", when = idle },
	{ id = "filter", title = "Filter processes", group = "Table", keys = "/", when = idle },
	{ id = "tree", title = "Show the process tree", group = "Table", keys = "t", when = idle },
	{ id = "group", title = "Group processes by name", group = "Table", keys = "g", when = idle },
	{ id = "fold", title = "Fold or open the branch", group = "Table", keys = "f", when = idle },
	{ id = "fold_all", title = "Fold every branch", group = "Table", keys = "[", when = idle },
	{ id = "open_all", title = "Open every branch", group = "Table", keys = "]", when = idle },
	{ id = "sort_next", title = "Sort by the next column", group = "Table", keys = ">", when = idle },
	{ id = "sort_prev", title = "Sort by the previous column", group = "Table", keys = "<", when = idle },
	{ id = "reverse", title = "Reverse the order", group = "Table", keys = "r", when = idle },
	{ id = "mark", title = "Mark or unmark the process", group = "Table", keys = "m", when = idle },
	{ id = "subtree", title = "Only this process and its children", group = "Table", keys = "o", when = idle },
})

-- --- The page -----------------------------------------------------------------------------------------------------

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

local function render(size, now, reduce_motion)
	u:begin(size, now, reduce_motion)
	S.now, S.width, S.height = now, size.w, size.h
	S.layout = layout_of(size.w)
	S.short = size.h < SIZE.short
	if not S.boot then
		return u:finish(booting())
	end
	local root = b.div():size_full():col():bg("background"):color("foreground"):family("terminal")
	root:child(overview.view())
	root:child(filter.view())
	root:child(b.div():grow(1):min_h(0):col():child(tab.view(size.w)))
	root:child(status.view())
	cmds:write(u, root)
	return u:finish(root)
end

-- --- What the person does ---------------------------------------------------------------------------------------

local function command(id)
	if id == "pause" then
		act.toggle_pause()
	elseif id == "filter" then
		return filter.focus()
	elseif id == "tree" then
		tab.mode("tree")
	elseif id == "group" then
		tab.mode("group")
	elseif id == "fold" then
		tab.fold()
	elseif id == "fold_all" then
		tab.fold_all(false)
	elseif id == "open_all" then
		tab.fold_all(true)
	elseif id == "sort_next" then
		tab.sort_step(1)
	elseif id == "sort_prev" then
		tab.sort_step(-1)
	elseif id == "reverse" then
		tab.reverse()
	elseif id == "mark" then
		tab.mark()
	elseif id == "subtree" then
		tab.subtree()
	end
	return true
end

-- What an action of a component comes to.
local function on_action(action)
	if action.from == "status" then
		status.action(action)
		return true
	end
	local reply = tab.action(action)
	if reply == nil then
		reply = filter.action(action)
	end
	if reply == nil and action.from == "commands" and action.kind == "command" then
		reply = command(action.id)
	end
	if reply == nil then
		reply = true
	end
	return reply
end

-- A key no component took: the interval.
local function key(event)
	if u.holder ~= "page" then
		return nil
	end
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
	local reply, actions, taken = u:handle(event)
	for _, action in actions do
		reply = b.merge(reply, on_action(action))
	end
	local kind = event.type
	if kind == "move" then
		tab.pointer(tab.contains({ x = event.x, y = event.y }))
	elseif kind == "leave" then
		tab.pointer(false)
	end
	if not taken then
		if kind == "key" then
			reply = b.merge(reply, key(event))
		elseif kind == "press" then
			reply = b.merge(reply, tab.event(event))
		end
	end
	return kit.take(reply)
end

-- --- Starting ----------------------------------------------------------------------------------------------------------

kit.call("boot", {}, function(boot, err)
	if err then
		S.boot_error = err.message or err.code
		return true
	end
	S.boot = boot
	status.start(boot.sampling)
	overview.start()
	tab.start()
	return true
end)

return {
	render = render,
	settle = function()
		tab.settle()
		return u:settle()
	end,
	handle = handle,
}
