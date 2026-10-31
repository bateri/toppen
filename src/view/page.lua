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
local detail = require("./parts/detail")
local actions = require("./parts/actions")
local machine = require("./parts/machine")
local ports = require("./parts/ports")
local commands = require("./parts/commands")
local help = require("./parts/help")
local palette = require("./parts/palette")
local settings = require("./parts/settings")

local b, u, S, SIZE, act, lib = kit.b, kit.u, kit.S, kit.SIZE, kit.act, kit.lib

-- --- The commands --------------------------------------------------------------------------------------------
--
-- What can be done, once (parts/commands.lua): the keys, the host's list, the palette, the help and
-- the status line are made of it. A key with a letter is the page's only while the keyboard is not
-- in a field. The conditions are named in the table and given here.

commands.when = {
	idle = function()
		return u.holder == "page"
	end,
	-- A command about a process: the keyboard is not in a field, and there is a process to be about.
	process = function()
		return u.holder == "page" and detail.target() ~= nil
	end,
	ports = function()
		return u.holder == "page" and ports.state.open
	end,
	-- The GPU's card is there only where the machine has a GPU.
	gpu = function()
		return u.holder == "page" and S.boot ~= nil and S.boot.features.gpu
	end,
}

local cmds = lib.commands(commands.for_library())

-- The keys changed (the settings): the host is told the commands as they are now.
act.keys_changed = function()
	cmds = lib.commands(commands.for_library())
end

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
	S.inner = size.w - 2 * SIZE.pad
	S.layout = layout_of(size.w)
	S.short = size.h < SIZE.short
	if not S.boot then
		return u:finish(booting())
	end
	local root = b.div():size_full():col():bg("background"):color("foreground"):family("terminal")
	root:child(overview.view())
	-- What the page is for sits in one panel under the cards, of their ground and corners: the table
	-- with its bar, the ports, or the details of a card.
	local panel = b.div():grow(1):min_h(0):col():rounded(SIZE.radius):bg("raised1"):clip()
	local card = machine.view()
	if card ~= nil then
		panel:child(card)
	elseif ports.state.open then
		panel:child(ports.view())
	else
		panel:child(filter.view())
		panel:child(b.div():grow(1):min_h(0):col():child(detail.view(tab.view(detail.table_width(S.inner)))))
	end
	root:child(b.div():grow(1):min_h(0):col():px(SIZE.pad):pt(SIZE.gap):pb(SIZE.pad):child(panel))
	root:child(status.view())
	actions.view(root)
	help.view(root)
	settings.view(root)
	palette.view(root)
	cmds:write(u, root)
	return u:finish(root)
end

-- --- What the person does ---------------------------------------------------------------------------------------

local function command(id, near)
	if id == "pause" then
		act.toggle_pause()
	elseif id == "filter" then
		machine.close()
		if ports.state.open then
			return u:focus(u:name("ports-filter", "field"))
		end
		return filter.focus()
	elseif id == "interval_faster" then
		act.step_interval(-1)
	elseif id == "interval_slower" then
		act.step_interval(1)
	elseif id == "palette" then
		palette.show()
	elseif id == "help" then
		help.show()
	elseif id == "settings" then
		settings.show()
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
	elseif string.sub(id, 1, 5) == "card_" then
		machine.toggle(string.sub(id, 6))
	elseif id == "range_next" then
		machine.next_range()
	elseif id == "ports" then
		machine.close()
		ports.toggle()
	elseif ports.command(id) then
		return true
	else
		local reply = actions.command(id, near)
		if reply ~= nil then
			return reply
		end
	end
	return true
end

act.run = command

-- What an action of a component comes to.
local function on_action(action)
	if action.from == "status" then
		status.action(action)
		return true
	end
	local from, kind = action.from, action.kind
	local reply = tab.action(action)
	if reply == nil then
		reply = filter.action(action)
	end
	if reply == nil then
		reply = machine.action(action)
	end
	if reply == nil then
		reply = ports.action(action)
	end
	if reply == nil then
		reply = detail.action(action)
	end
	if reply == nil then
		reply = actions.action(action)
	end
	if reply == nil then
		reply = palette.action(action)
	end
	if reply == nil then
		reply = help.action(action)
	end
	if reply == nil then
		reply = settings.action(action)
	end
	if reply == nil and from == "commands" and kind == "command" then
		reply = command(action.id)
	elseif reply == nil and (from == "row-menu" or from == "signal-menu" or from == "ports-menu") and kind == "menu" then
		reply = command(action.entry)
	elseif reply == nil and string.sub(from, 1, 3) == "do-" and kind == "press" then
		reply = command(string.sub(from, 4), from)
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
	if name == "escape" and machine.state.open ~= nil then
		machine.close()
		return true
	elseif name == "escape" and ports.state.open then
		ports.hide()
		return true
	elseif name == "escape" and detail.state.open then
		detail.close()
		return true
	elseif machine.key(event) then
		return true
	end
	return nil
end

local function handle(event)
	-- A command waits for its key in the settings: the next key is its, whatever the key is.
	if event.type == "key" and settings.capturing() then
		return kit.take(settings.capture(event))
	end
	local reply, actions, taken = u:handle(event)
	for _, action in actions do
		reply = b.merge(reply, on_action(action))
	end
	local kind = event.type
	if kind == "move" then
		tab.pointer(tab.contains({ x = event.x, y = event.y }))
		if machine.pointer(event.x, event.y) then
			reply = b.merge(reply, true)
		end
	elseif kind == "leave" then
		tab.pointer(false)
		if machine.leave() then
			reply = b.merge(reply, true)
		end
	end
	if not taken then
		if kind == "key" then
			reply = b.merge(reply, key(event))
		elseif kind == "press" then
			local pressed = tab.event(event)
			if pressed == nil then
				pressed = detail.event(event)
			end
			if pressed == nil then
				pressed = machine.event(event)
			end
			reply = b.merge(reply, pressed)
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
	settings.start(boot)
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
