-- The listening ports: the sockets that wait for connections (or are bound to a port, over UDP) and the
-- process that holds each, in place of the process table until the person goes back. The program
-- scans and orders and filters (`ports`); to change the filter or the order the page subscribes again.
-- The commands about a process (a signal, a new priority, a shell) are of the process of the row.

local kit = require("../lib/kit")
local format = require("../lib/format")
local filter = require("./filter")
local detail = require("./detail")
local actions = require("./actions")
local commands = require("./commands")

local b, u, S, act = kit.b, kit.u, kit.S, kit.act

local ports = {}

local PAD = 10
local BAR = 44
local ROW = 22

-- What the view keeps: what the person typed, the order, and what the program told.
local P = {
	open = false,
	-- The field's state.
	field = { value = "" },
	sort = "port",
	descending = false,
	got = false,
	rows = {},
	total = 0,
	matched = 0,
	sub = nil,
	-- Whether the keyboard was put in the grid yet.
	focused = false,
	-- The row the cursor is on, by what names it, so that it stays on its socket as the list changes.
	selected = nil,
}
ports.state = P

-- The grid's own state.
local G = { cursor = nil, widths = {}, sort = nil }

local COLUMNS = {
	{ key = "port", title = "Port", width = 80, align = "end", priority = 9 },
	{ key = "proto", title = "Proto", width = 70, priority = 8 },
	{ key = "address", title = "Address", width = 190, priority = 5 },
	{ key = "pid", title = "PID", width = 80, align = "end", priority = 7 },
	{ key = "process", title = "Process", width = 220, priority = 9 },
	{ key = "user", title = "User", width = 120, priority = 4 },
	{ key = "command", title = "Command", width = 420, priority = 0 },
}
local SORTS = { port = true, address = true, pid = true, process = true, user = true }

local function key_of(row)
	return string.format("%s/%d/%s/%s", row.proto, row.port, row.addr, tostring(row.pid))
end

local function row_at(place)
	return P.rows[place]
end

-- --- Asking the program --------------------------------------------------------------------------------

local function subscribe()
	if P.sub ~= nil then
		b.cancel(P.sub)
	end
	P.sub = b.subscribe("ports", { filter = P.field.value, sort = P.sort, descending = P.descending }, function(told)
		P.got = true
		P.rows, P.total, P.matched = told.rows, told.total, told.matched
		-- The cursor follows its socket; where that is gone it keeps its place.
		local at = nil
		if P.selected ~= nil then
			for index, row in P.rows do
				if key_of(row) == P.selected then
					at = index
				end
			end
		end
		if G.cursor == nil and #P.rows > 0 then
			G.cursor = { row = 1, column = 1 }
			at = at or 1
		end
		if G.cursor ~= nil then
			G.cursor.row = math.max(1, math.min(at or G.cursor.row, math.max(#P.rows, 1)))
			local row = P.rows[G.cursor.row]
			P.selected = row and key_of(row) or nil
		end
		if not P.focused and #P.rows > 0 then
			P.focused = true
			kit.gather(u:focus(u:name("ports", "body")))
		end
		return kit.take(true)
	end)
end

function ports.show()
	if P.open then
		return
	end
	P.open, P.got, P.focused = true, false, false
	subscribe()
end

function ports.hide()
	if not P.open then
		return
	end
	P.open = false
	if P.sub ~= nil then
		b.cancel(P.sub)
		P.sub = nil
	end
	kit.gather(u:focus(u:name("procs", "body")))
end

function ports.toggle()
	if P.open then
		ports.hide()
	else
		ports.show()
	end
end

-- --- The row under the cursor ---------------------------------------------------------------------------

local function current()
	if G.cursor == nil then
		return nil
	end
	return row_at(G.cursor.row)
end

local function mine(user)
	local person = S.boot.person
	return person.root or person.name == user
end

-- The process of the row under the cursor, as `detail.target` says it, while the ports are shown.
function ports.target()
	if not P.open then
		return nil
	end
	local row = current()
	if row == nil or row.pid == nil or row.name == nil then
		return nil
	end
	return {
		pid = row.pid,
		start = row.start,
		name = row.name,
		user = row.user,
		command = row.command or row.name,
		children = nil,
		nice = 0,
		mine = mine(row.user),
		cwd = nil,
		ended = false,
	}
end

-- The address of the row, as it is written to be reached: `127.0.0.1:3000`, `[::1]:80`, `*:5353`.
function ports.address()
	local row = current()
	if row == nil then
		return nil
	end
	local host = row.addr
	if string.find(host, ":", 1, true) ~= nil then
		host = "[" .. host .. "]"
	end
	return string.format("%s:%d", host, row.port)
end

function ports.port()
	local row = current()
	return row and tostring(row.port) or nil
end

-- Shows the process of the row in the table, with its details.
function ports.show_process()
	local row = current()
	if row == nil or row.pid == nil or row.name == nil then
		return false
	end
	ports.hide()
	filter.set("pid:" .. row.pid)
	detail.show(row.pid, row.start, row.name, true)
	return true
end

-- --- The cells ---------------------------------------------------------------------------------------------

local function right(words)
	return b.text(words):truncate():min_w(0):grow(1):align("end")
end

local function cell(place, column)
	local row = row_at(place)
	if row == nil then
		return nil
	end
	local key = column.key
	if key == "port" then
		return right(tostring(row.port))
	elseif key == "proto" then
		return row.proto
	elseif key == "address" then
		return row.addr
	elseif key == "pid" then
		return right(row.pid and tostring(row.pid) or "")
	elseif key == "process" then
		if row.name == nil then
			return b.text(row.pid and "not visible" or "unknown"):color("quiet"):truncate():min_w(0)
		end
		return row.name
	elseif key == "user" then
		return row.user ~= nil and kit.user_shown(row.user) or ""
	elseif key == "command" then
		return b.text(row.command or ""):color("dim"):truncate():min_w(0)
	end
	return nil
end

local function columns_in(room)
	local list = {}
	for _, column in COLUMNS do
		list[#list + 1] = column
	end
	local function sum()
		local total = 0
		for _, column in list do
			if column.key ~= "command" then
				total += column.width
			end
		end
		return total
	end
	local command = nil
	for index, column in list do
		if column.key == "command" then
			command = table.remove(list, index)
			break
		end
	end
	while sum() > room and #list > 3 do
		local worst, at = nil, nil
		for index, column in list do
			if worst == nil or column.priority < worst.priority then
				worst, at = column, index
			end
		end
		table.remove(list, at)
	end
	local out = {}
	for _, column in list do
		out[#out + 1] = { key = column.key, title = column.title, align = column.align, width = G.widths[column.key] or column.width }
	end
	if command ~= nil and room - sum() >= 160 then
		out[#out + 1] = { key = "command", title = "Command", width = G.widths.command or math.floor(room - sum()) }
	end
	return out
end

-- --- Drawing ------------------------------------------------------------------------------------------------

local SCREENS = { { key = "processes", title = "Processes" }, { key = "ports", title = "Ports" } }

-- The two views of the page side by side as a switch: the processes and the ports.
function ports.switch()
	return u:segments("screen", { value = P.open and "ports" or "processes", items = SCREENS })
end

local function count_words()
	if P.field.value ~= "" then
		return string.format("%d of %d listening sockets", P.matched, P.total)
	end
	return string.format("%d listening sockets", P.total)
end

local function bar()
	local row = b.div():row():items_center():gap(8):px(PAD):h(BAR):no_shrink()
	if S.width >= 560 then
		row:child(ports.switch())
	end
	local trail = {}
	if P.field.value ~= "" then
		trail[1] = u:icon_button("ports-clear", { icon = "close", label = "Clear the filter", keys = "escape" })
	end
	local field = u:input("ports-filter", {
		state = P.field,
		label = "Filter the ports",
		hint = "Filter: :3000, a name, a user",
		lead = kit.icon("search", 14, "quiet"),
		trail = trail,
		escape = true,
	})
	row:child(b.div():grow(1):min_w(0):max_w(420):child(field))
	row:child(b.text(count_words()):color("dim"):no_shrink())
	return row
end

function ports.view()
	if not P.open then
		return nil
	end
	local box = b.div():col():grow(1):min_h(0)
	box:child(bar())
	if not P.got then
		box:child(b.div():grow(1):items_center():justify_center():child(kit.spinner(18, "quiet")))
		return box
	end
	if #P.rows == 0 then
		local words = P.field.value ~= "" and "No listening socket holds all of that." or "Nothing is listening."
		box:child(b.div():grow(1):items_center():justify_center():child(b.text(words):color("dim")))
		return box
	end
	local columns = columns_in(S.inner - 2)
	if G.cursor ~= nil and G.cursor.column > #columns then
		G.cursor.column = #columns
	end
	G.sort = { column = P.sort, descending = P.descending }
	local blind = false
	for _, row in P.rows do
		if row.pid == nil or row.name == nil then
			blind = true
		end
	end
	box:child(b.div():grow(1):min_h(0):col():child(u:grid("ports", {
		columns = columns,
		count = #P.rows,
		cell = cell,
		state = G,
		numbers = false,
		-- A socket is acted on through its process, not a value in it: the table chooses rows.
		select = "row",
		row_height = ROW,
		head_height = ROW + 2,
	})))
	if blind then
		box:child(b.div():px(PAD):py(4):no_shrink():child(
			b.text("A socket with no process belongs to one that you may not look into; run toppen as root to see them."):color("quiet"):text_size(12):truncate():min_w(0)
		))
	end
	return box
end

-- --- Actions ---------------------------------------------------------------------------------------------------

local function change(text)
	P.field.value = text
	subscribe()
end

local function sort_by(key)
	if not SORTS[key] then
		return
	end
	if P.sort == key then
		P.descending = not P.descending
	else
		P.sort, P.descending = key, false
	end
	subscribe()
end

-- The menu of a row: the commands, as the keys do them.
local function row_menu(place, at)
	local row = row_at(place)
	if row == nil then
		return true
	end
	G.cursor = G.cursor or { row = place, column = 1 }
	G.cursor.row = place
	P.selected = key_of(row)
	local known = row.pid ~= nil and row.name ~= nil
	local owned = known and mine(row.user)
	local entries = {
		{ key = "port_show", title = "Show the process in the table", keys = "enter", enabled = known },
		"-",
		{ key = "signal_term", title = "End it (TERM)", enabled = owned },
		{ key = "signal_kill", title = "Kill it (KILL)", enabled = owned },
		{ key = "signal", title = "More signals", keys = commands.first("signal"), enabled = owned },
		"-",
		{ key = "copy_address", title = "Copy the address", keys = commands.first("copy_address") },
		{ key = "copy_port", title = "Copy the port" },
	}
	return u:menu("ports-menu", entries, { at = at })
end

-- An action of the view. Whether it was its own, and its reply.
function ports.action(action)
	local from, kind = action.from, action.kind
	if from == "ports-filter" then
		if kind == "change" then
			change(action.text)
		elseif kind == "submit" then
			return u:focus(u:name("ports", "body"))
		elseif kind == "escape" then
			if P.field.value ~= "" then
				change("")
			else
				return u:focus(u:name("ports", "body"))
			end
		end
		return true
	elseif from == "ports-clear" and kind == "press" then
		change("")
		return u:focus(u:name("ports-filter", "field"))
	elseif from == "screen" and kind == "select" then
		if action.key == "ports" then
			ports.show()
		else
			ports.hide()
		end
		return true
	elseif from == "ports" then
		if kind == "select" then
			local row = row_at(action.row)
			if row ~= nil then
				P.selected = key_of(row)
			end
		elseif kind == "sort" then
			sort_by(action.column)
		elseif kind == "open" then
			ports.show_process()
		elseif kind == "menu" then
			return row_menu(action.row, action.at)
		end
		return true
	end
	return nil
end

-- The commands of the view for the page's list.
-- Does the command `id` of this file; nil where it is not one of them.
function ports.command(id)
	if id == "ports" then
		ports.toggle()
		return true
	elseif id == "port_show" then
		ports.show_process()
		return true
	elseif id == "copy_address" or id == "copy_port" then
		local words = id == "copy_address" and ports.address() or ports.port()
		if words ~= nil then
			kit.gather({ copy = words })
			actions.toast(id == "copy_address" and "Copied the address." or "Copied the port.", "ok", 2)
		end
		return true
	end
	return nil
end

act.ports_target = ports.target
act.switch = ports.switch

return ports
