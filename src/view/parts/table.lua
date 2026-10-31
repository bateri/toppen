-- The process table: what the program says of the processes in view (`processes`), and the library's
-- grid that draws them. The page holds the rows of a window, not the processes: ordering, filtering,
-- folding and holding the order are the program's (`view`), and the page says how the person wants
-- the table and where they are looking.

local kit = require("../lib/kit")
local format = require("../lib/format")
local commands = require("./commands")

local b, u, S, act = kit.b, kit.u, kit.S, kit.act

local tab = {}

local ROW = 26
-- How far past the rows in view the program is asked for rows, so that a quick scroll lands on rows
-- that are already there; and how near the edge of what is held the view may come before more is asked.
local AHEAD = 120
local NEAR = 40
-- How long the order stays held after a key or a click, and after the pointer moved over the table,
-- seconds. A pointer that rests holds nothing: the table goes on sorting under it.
local KEYED = 3
local MOVED = 2

-- The columns the table can show, in order. `priority` is what stays when the page is narrow (the
-- least goes first); `fixed` never goes; `down` sorts largest first on the first press.
local COLUMNS = {
	{ key = "name", title = "Name", width = 240, priority = 9, fixed = true },
	{ key = "pid", title = "PID", width = 72, align = "end", priority = 8 },
	{ key = "user", title = "User", width = 100, priority = 4 },
	{ key = "cpu", title = "CPU", width = 84, align = "end", priority = 9, down = true },
	{ key = "memory", title = "Memory", width = 96, align = "end", priority = 8, down = true },
	{ key = "threads", title = "Threads", width = 78, align = "end", priority = 3, down = true },
	{ key = "state", title = "State", width = 80, priority = 2 },
	{ key = "time", title = "Time", width = 92, align = "end", priority = 2, down = true },
	{ key = "virt", title = "Virtual", width = 100, align = "end", priority = 1, down = true },
	{ key = "nice", title = "Nice", width = 60, align = "end", priority = 1 },
	{ key = "read", title = "Read", width = 96, align = "end", priority = 1, down = true },
	{ key = "write", title = "Write", width = 96, align = "end", priority = 1, down = true },
	{ key = "command", title = "Command", width = 420, priority = 0 },
}
local BY_KEY = {}
for _, column in COLUMNS do
	BY_KEY[column.key] = column
end

-- What the table keeps: what the program last said, and what the person did to the table here.
local T = {
	got = false,
	view = { sort = "cpu", descending = true, mode = "flat", filter = "", hold = false, root = nil, error = nil },
	total = 0,
	matched = 0,
	visible = 0,
	moved = 0,
	held = false,
	-- The index (from 0) of rows[1] among all the rows, and the rows told.
	first = 0,
	rows = {},
	-- The row the cursor is on, as the program names it: { pid =, start = } or { group = }.
	selected = nil,
	-- The columns drawn last, and the ones the person hid.
	shown = {},
	hidden = { virt = true, nice = true, read = true, write = true },
	-- Until when the order is held: a key, a click or the pointer moving lately.
	keyed = 0,
	-- A timer is set to let go of the order.
	waking = false,
	-- The window last asked for, to ask it once.
	window = nil,
	-- Whether the keyboard was put in the table yet.
	focused = false,
}
tab.state = T

-- The grid's own state: the cursor, the widths the person dragged, the sort it draws.
local G = { cursor = nil, widths = {}, sort = nil }

local function row_at(place)
	return T.rows[place - T.first]
end

-- A row as the program names it in `view`.
local function ref(row)
	if row.group then
		return { group = row.name }
	end
	return { pid = row.pid, start = row.start }
end

local function same(a, b_)
	if a == nil or b_ == nil then
		return a == b_
	end
	return a.group == b_.group and a.pid == b_.pid and a.start == b_.start
end

-- --- Asking the program --------------------------------------------------------------------------------

-- Tells the program how the table is to be, and takes what it says it is.
local function set(params)
	kit.call("view", params, function(result, err)
		if result then
			T.view = result
		end
		return true
	end)
end

local function sync_hold()
	local now = b.time.now()
	local want = now < T.keyed
	if want ~= T.view.hold then
		T.view.hold = want
		set({ hold = want })
	end
	-- One timer at a time, set again while a later key or move keeps the order held.
	if want and not T.waking then
		T.waking = true
		b.time.after(T.keyed - now + 0.05, function()
			T.waking = false
			sync_hold()
			return kit.take(true)
		end)
	end
end

-- The order stays where it is for `seconds` from now, or for longer where it already does.
local function hold_for(seconds)
	T.keyed = math.max(T.keyed, b.time.now() + seconds)
	sync_hold()
end

-- A key or a click on a row: the order stays where it is for a few seconds.
local function keyed()
	hold_for(KEYED)
end

-- The pointer moved over the table: the rows stay where they are while it aims, and not for longer.
function tab.pointer(over)
	if over then
		hold_for(MOVED)
	end
end

local function ask_window(first, last)
	local key = first .. ":" .. last
	if T.window ~= key then
		T.window = key
		set({ window = { first = first - 1, count = last - first + 1 } })
	end
end

-- The window to ask for, where the rows near the ones the grid has made are not all held: the rows in
-- view and `AHEAD` more on each side, so that a scroll finds its rows already told. Nil where there is
-- nothing to ask, or it was asked already.
local function missing()
	if not T.got then
		return nil
	end
	local first, last = b.list_window(u:name("procs", "body"))
	if first == nil then
		return nil
	end
	local count = math.max(T.visible, 0)
	local want = false
	for place = math.max(1, first - NEAR), math.min(count, last + NEAR) do
		if row_at(place) == nil then
			want = true
			break
		end
	end
	if not want then
		return nil
	end
	local from, to = math.max(1, first - AHEAD), math.max(math.min(count, last + AHEAD), first)
	if T.window == from .. ":" .. to then
		return nil
	end
	return from, to
end

local function fill()
	local first, last = missing()
	if first ~= nil then
		ask_window(first, last)
	end
end

-- The page was laid out: the rows that came into view are asked for, in the next frame (a layout has
-- no reply to ask in). A timer only where there is something to ask, or the page would never settle.
function tab.settle()
	if missing() ~= nil then
		b.time.after(0, function()
			fill()
			return kit.take(nil)
		end)
	end
end

-- --- What the program told ---------------------------------------------------------------------------------

function tab.told(told)
	T.got = true
	T.total, T.matched, T.visible = told.total, told.matched, told.visible
	T.moved, T.held, T.first, T.rows = told.moved, told.held, told.first, told.rows
	T.view = told.view
	G.sort = { column = told.view.sort, descending = told.view.descending }
	if T.selected ~= nil then
		if told.selectedIndex ~= nil then
			-- The cursor follows its process wherever the order takes it. The same table is changed, not
			-- a new one made: the grid scrolls to a cursor it has not seen.
			if G.cursor == nil then
				G.cursor = { row = told.selectedIndex + 1, column = 1 }
			else
				G.cursor.row = told.selectedIndex + 1
			end
		elseif G.cursor ~= nil then
			-- It ended, or a fold hides it: the cursor stays where it was, on whatever is there.
			G.cursor.row = math.max(1, math.min(G.cursor.row, told.visible))
			local row = row_at(G.cursor.row)
			if row ~= nil and not same(T.selected, ref(row)) then
				T.selected = ref(row)
				set({ selected = T.selected })
			end
		end
	end
	if G.cursor == nil and T.selected == nil and told.first == 0 and told.rows[1] ~= nil then
		-- The grid takes any key as a step to its first cell while it has no cursor: it starts on the
		-- first row, as every list does.
		G.cursor = { row = 1, column = 1 }
		T.selected = ref(told.rows[1])
		set({ selected = T.selected })
		-- The settings ask for the panel of a process to be open from the start.
		if S.details_at_start and act.detail_open ~= nil then
			S.details_at_start = false
			act.detail_open(told.rows[1])
		end
	end
	if not T.focused then
		T.focused = true
		kit.gather(u:focus(u:name("procs", "body")))
	end
end

-- The table as the settings keep it: how it is ordered and shown, and which columns are not.
function tab.snapshot()
	local hidden = {}
	for _, column in COLUMNS do
		if T.hidden[column.key] then
			hidden[#hidden + 1] = column.key
		end
	end
	return { sort = T.view.sort, descending = T.view.descending, mode = T.view.mode, hidden = hidden }
end

-- The table the settings ask for, before the program's first word on it.
function tab.apply_settings(values)
	local params = {}
	if values.sort ~= nil and BY_KEY[values.sort] ~= nil then
		T.view.sort = values.sort
		params.sort = values.sort
		T.view.descending = values.descending ~= false
		params.descending = T.view.descending
		G.sort = { column = T.view.sort, descending = T.view.descending }
	end
	if values.mode ~= nil then
		T.view.mode = values.mode
		params.mode = values.mode
	end
	if values.hidden ~= nil then
		T.hidden = {}
		for _, key in values.hidden do
			if BY_KEY[key] ~= nil and not BY_KEY[key].fixed then
				T.hidden[key] = true
			end
		end
	end
	if next(params) ~= nil then
		set(params)
	end
end

-- The columns the table can show, for the settings.
function tab.columns()
	return COLUMNS
end

function tab.start()
	b.subscribe("processes", {}, function(told)
		tab.told(told)
		return kit.take(true)
	end)
end

-- --- The cells -----------------------------------------------------------------------------------------------

-- A text of a row, in the tone of its state: dim once the process has ended. The numbers of a column
-- that ends at the right are set against its edge (the grid only moves a cell's box, and the words
-- grow to fill it).
local function words_of(row, words, at_end)
	local text = b.text(words):color(row.exited and "quiet" or (at_end and "dim" or "foreground")):truncate():min_w(0)
	if at_end then
		return b.div():row():items_center():justify_end():grow(1):min_w(0):child(text)
	end
	return text
end

-- The words with the ranges `[from, to)` of characters marked, as the filter found them.
local function marked(words, ranges, base, weight)
	if ranges == nil or #ranges == 0 then
		local text = b.text(words):color(base):truncate():min_w(0)
		if weight ~= nil then
			text:weight(weight)
		end
		return text
	end
	local line = b.div():row():items_center():min_w(0):clip()
	local at = 1
	for _, range in ranges do
		local from = utf8.offset(words, range[1] + 1)
		local to = utf8.offset(words, range[2] + 1)
		if from ~= nil and to ~= nil and from >= at and to > from then
			if from > at then
				line:child(b.text(string.sub(words, at, from - 1)):color(base):no_shrink())
			end
			line:child(b.div():bg(b.alpha("accent", 0.32)):rounded(2)
				:child(b.text(string.sub(words, from, to - 1)):color("foreground"):no_shrink()))
			at = to
		end
	end
	if at <= #words then
		line:child(b.text(string.sub(words, at)):color(base):no_shrink())
	end
	return line
end

local function toggle_name(row)
	if row.group then
		return "procs-fold/g/" .. row.name
	end
	return "procs-fold/p/" .. row.pid .. "/" .. row.start
end

local function name_cell(row)
	local base = row.exited and "quiet" or "foreground"
	local box = b.div():row():items_center():gap(2):grow(1):min_w(0):clip()
	if T.view.mode ~= "flat" then
		if row.depth > 0 then
			box:child(b.div():w(row.depth * 14):h(1):no_shrink())
		end
		if row.children > 0 then
			box:child(b.div():press(toggle_name(row)):role("button")
				:label((row.folded and "Open " or "Fold ") .. row.name)
				:w(16):h(ROW - 4):no_shrink():items_center():justify_center():rounded(3)
				:child(kit.icon(row.folded and "chevron-right" or "chevron-down", 13, "dim")))
		else
			box:child(b.div():w(16):h(1):no_shrink())
		end
	end
	if not row.group and S.marks[row.pid] then
		box:child(b.div():w(7):h(7):rounded(4):bg("accent"):mr(4):no_shrink())
	end
	box:child(marked(row.name, row.hits and row.hits.name, base, 600))
	if row.children > 0 and (row.group or row.folded) then
		box:child(b.text((row.group and "×" or "+") .. row.children):color("quiet"):no_shrink())
	end
	return box
end

-- A number set against the right edge of its cell, in `role`; quiet once the process has ended.
local function number_cell(row, words, role)
	return b.div():row():items_center():justify_end():grow(1):min_w(0)
		:child(b.text(words):color(row.exited and "quiet" or role):truncate():min_w(0))
end

-- The processor a process takes, as a share of one processor: quiet when it is next to nothing, and
-- warmer as it takes more.
local function cpu_role(share)
	if share < 0.5 then
		return "quiet"
	elseif share >= 90 then
		return "error"
	elseif share >= 50 then
		return "warning"
	end
	return "foreground"
end

-- The memory a process holds: quiet when it is little, a warning where it is a fifth of the machine's.
local function memory_role(bytes)
	local total = S.boot.machine.memory
	if bytes < 16 * 1024 * 1024 then
		return "quiet"
	elseif total > 0 and bytes / total >= 0.2 then
		return "warning"
	end
	return "foreground"
end

-- The state of a process as a word, in the tone of what it means.
local STATE_WORDS = {
	R = { "running", "success" },
	S = { "sleeping", "quiet" },
	I = { "idle", "quiet" },
	D = { "disk", "warning" },
	T = { "stopped", "warning" },
	Z = { "zombie", "error" },
}

local function state_cell(row)
	if row.group then
		return ""
	end
	local known = STATE_WORDS[row.state]
	local words, role = row.state, "dim"
	if known ~= nil then
		words, role = known[1], known[2]
	end
	return b.text(words):color(row.exited and "quiet" or role):truncate():min_w(0)
end

local function cell(place, column)
	local row = row_at(place)
	if row == nil then
		return nil
	end
	local key = column.key
	if key == "name" then
		return name_cell(row)
	elseif key == "pid" then
		return words_of(row, row.group and "" or tostring(row.pid), true)
	elseif key == "user" then
		return words_of(row, kit.user_shown(row.user))
	elseif key == "cpu" then
		return number_cell(row, string.format("%.1f%%", row.cpu), cpu_role(row.cpu))
	elseif key == "memory" then
		return number_cell(row, format.short_bytes(row.mem), memory_role(row.mem))
	elseif key == "threads" then
		return words_of(row, tostring(row.threads), true)
	elseif key == "state" then
		return state_cell(row)
	elseif key == "time" then
		return words_of(row, format.cpu_time(row.time), true)
	elseif key == "virt" then
		return words_of(row, format.short_bytes(row.virt), true)
	elseif key == "nice" then
		return words_of(row, row.group and "" or tostring(row.nice), true)
	elseif key == "read" then
		return words_of(row, row.read >= 1 and format.rate(row.read) or "", true)
	elseif key == "write" then
		return words_of(row, row.write >= 1 and format.rate(row.write) or "", true)
	elseif key == "command" then
		return marked(row.command, row.hits and row.hits.command, row.exited and "quiet" or "dim")
	end
	return nil
end

-- --- The columns -----------------------------------------------------------------------------------------------

-- The columns drawn in `room` points: those the person did not hide, the ones of the least priority
-- gone first where they do not fit; the command takes what is left, if that is enough to read.
local function columns_in(room)
	local list = {}
	for _, column in COLUMNS do
		if not T.hidden[column.key] then
			list[#list + 1] = column
		end
	end
	-- The name gives way before a column goes: down to this, unless the person sized it.
	local name_least = 150
	local name_now = nil
	local function width(column)
		if G.widths[column.key] ~= nil then
			return math.max(G.widths[column.key], 56)
		end
		if column.key == "name" and name_now ~= nil then
			return name_now
		end
		return math.max(column.width, 56)
	end
	local function sum()
		local total = 0
		for _, column in list do
			if column.key ~= "command" then
				total += width(column)
			end
		end
		return total
	end
	local has_command = false
	for index, column in list do
		if column.key == "command" then
			has_command = true
			table.remove(list, index)
			break
		end
	end
	while sum() > room do
		local over = sum() - room
		if G.widths.name == nil and width(BY_KEY.name) > name_least then
			name_now = math.max(name_least, width(BY_KEY.name) - over)
		else
			local worst, at = nil, nil
			for index, column in list do
				if not column.fixed and (worst == nil or column.priority < worst.priority) then
					worst, at = column, index
				end
			end
			if worst == nil then
				break
			end
			table.remove(list, at)
		end
	end
	local out = {}
	for index, column in list do
		out[index] = { key = column.key, title = column.title, align = column.align, width = width(column) }
	end
	if has_command and room - sum() >= 160 then
		out[#out + 1] = { key = "command", title = "Command", width = G.widths.command or math.floor(room - sum()) }
	end
	return out
end

-- --- Actions of the table ------------------------------------------------------------------------------------

local function toggle_fold(row)
	set({ toggle = ref(row) })
end

local function select_row(row)
	T.selected = ref(row)
	set({ selected = T.selected })
	keyed()
	if act.follow ~= nil then
		act.follow(row)
	end
end

local function selected_row()
	if G.cursor == nil then
		return nil
	end
	return row_at(G.cursor.row)
end

-- The process under the cursor, or nil where it is on a group or on nothing.
function tab.current()
	local row = selected_row()
	if row == nil or row.group then
		return nil
	end
	return row
end

function tab.view_mode()
	return T.view.mode
end

-- Moves the cursor to a process, where the list has it; the program says where that is.
function tab.select_key(pid, start)
	T.selected = { pid = pid, start = start }
	set({ selected = T.selected })
end

-- A place by the cursor's row for a menu to open at: under the row, a little in from its left edge.
function tab.anchor()
	local box = G.cursor and b.layout.bounds(u:name("procs", "row", G.cursor.row))
	if box == nil then
		box = b.layout.bounds("procs")
	end
	if box == nil then
		return { x = 40, y = 80 }
	end
	return { x = box.x + 48, y = box.y + box.h }
end

local function toggle_mark(row)
	if row == nil or row.group then
		return
	end
	S.marks[row.pid] = (not S.marks[row.pid]) or nil
end

function tab.mark()
	toggle_mark(selected_row())
end

function tab.fold()
	local row = selected_row()
	if row ~= nil and row.children > 0 then
		toggle_fold(row)
	end
end

function tab.fold_all(open)
	set({ fold = open and "none" or "all" })
end

-- Tree and groups are modes of the same table: the key of the mode in force goes back to the flat list.
function tab.mode(mode)
	if T.view.mode == mode then
		mode = "flat"
	end
	T.view.mode = mode
	set({ mode = mode })
end

function tab.set_mode(mode)
	T.view.mode = mode
	set({ mode = mode })
end

-- Narrows the table to the process under the cursor and what is below it, or leaves it.
function tab.subtree()
	if T.view.root ~= nil then
		set({ root = b.json.null })
		return
	end
	local row = selected_row()
	if row ~= nil and not row.group then
		T.root_name = row.name
		set({ root = { pid = row.pid, start = row.start } })
	end
end

function tab.leave_subtree()
	set({ root = b.json.null })
end

-- Lets go of the held order: the table sorts again.
function tab.release()
	T.keyed = 0
	T.view.hold = false
	set({ hold = false })
end

local function sort_by(key)
	local column = BY_KEY[key]
	if column == nil then
		return
	end
	local descending
	if T.view.sort == key then
		descending = not T.view.descending
	else
		descending = column.down == true
	end
	T.view.sort, T.view.descending = key, descending
	G.sort = { column = key, descending = descending }
	set({ sort = key, descending = descending })
end

-- A step to the next sortable column on the right (`1`) or the left (`-1`), as the table shows them.
function tab.sort_step(direction)
	local order = {}
	for _, column in T.shown do
		order[#order + 1] = column.key
	end
	local at = 1
	for index, key in order do
		if key == T.view.sort then
			at = index
		end
	end
	local key = order[((at - 1 + direction) % #order) + 1]
	if key ~= nil then
		sort_by(key)
	end
end

function tab.reverse()
	T.view.descending = not T.view.descending
	G.sort = { column = T.view.sort, descending = T.view.descending }
	set({ descending = T.view.descending })
end

local function row_menu(place, at)
	local row = row_at(place)
	if row == nil then
		return true
	end
	select_row(row)
	if not row.group and act.row_menu ~= nil then
		return act.row_menu(row, at)
	end
	local entries = {}
	if row.children > 0 then
		local what = row.group and "group" or "branch"
		entries[#entries + 1] = { key = "fold", title = (row.folded and "Open " or "Fold ") .. what, keys = commands.first("fold") }
	end
	if not row.group then
		entries[#entries + 1] = { key = "mark", title = S.marks[row.pid] and "Unmark" or "Mark", keys = commands.first("mark") }
		entries[#entries + 1] = {
			key = "subtree",
			title = T.view.root ~= nil and "Show every process" or "Only this process and its children",
			keys = commands.first("subtree"),
		}
	end
	if #entries == 0 then
		return true
	end
	return u:menu("row-menu", entries, { at = at })
end

-- The columns' own menu: a click on the head with the right button.
function tab.columns_menu(at)
	local entries = {}
	for _, column in COLUMNS do
		if not column.fixed then
			local hidden = T.hidden[column.key]
			entries[#entries + 1] = { key = column.key, title = (hidden and "Show " or "Hide ") .. column.title }
		end
	end
	return u:menu("columns-menu", entries, { at = at })
end

-- An action of the grid or of its menus. Whether it was the table's, and its reply.
function tab.action(action)
	local from, kind = action.from, action.kind
	if from == "procs" then
		if kind == "select" then
			local row = row_at(action.row)
			if row == nil then
				return true
			end
			select_row(row)
		elseif kind == "sort" then
			sort_by(action.column)
		elseif kind == "open" then
			-- Enter and a double click show the process; a group is opened or folded.
			local row = row_at(action.row)
			if row ~= nil and row.group then
				toggle_fold(row)
			elseif row ~= nil and act.detail_open ~= nil then
				act.detail_open(row)
			end
		elseif kind == "menu" then
			return row_menu(action.row, action.at)
		elseif kind == "need" then
			fill()
		end
		return true
	elseif from == "columns-menu" and kind == "menu" then
		T.hidden[action.entry] = (not T.hidden[action.entry]) or nil
		return true
	end
	return nil
end

-- An event no component took: a fold arrow pressed, a right press on the head of a column.
function tab.event(event)
	if event.type ~= "press" then
		return nil
	end
	-- What the press landed on: the library has not taken it, so it is read from the layout.
	local over = b.layout.hit({ x = event.x, y = event.y })
	if over == nil then
		return nil
	end
	if event.button == "right" and string.sub(over, 1, 11) == "procs/head/" then
		return tab.columns_menu({ x = event.x, y = event.y })
	end
	if (event.button or "left") == "left" then
		local pid, start = string.match(over, "^procs%-fold/p/(%d+)/(%d+)$")
		if pid ~= nil then
			set({ toggle = { pid = tonumber(pid), start = tonumber(start) } })
			return true
		end
		local group = string.match(over, "^procs%-fold/g/(.+)$")
		if group ~= nil then
			set({ toggle = { group = group } })
			return true
		end
	end
	return nil
end

-- --- Drawing -------------------------------------------------------------------------------------------------------

-- Whether the pointer at `at` is over the table.
function tab.contains(at)
	local box = b.layout.bounds("procs")
	return box ~= nil and at ~= nil and at.x >= box.x and at.x < box.x + box.w and at.y >= box.y and at.y < box.y + box.h
end

function tab.summary()
	return T
end

function tab.view(width)
	if not T.got then
		return b.div():grow(1):items_center():justify_center():child(kit.spinner(18, "quiet"))
	end
	local columns = columns_in(width - 2)
	T.shown = columns
	if G.cursor ~= nil and G.cursor.column > #columns then
		G.cursor.column = #columns
	end
	return u:grid("procs", {
		columns = columns,
		count = T.visible,
		cell = cell,
		state = G,
		numbers = false,
		row_height = ROW,
		head_height = ROW + 4,
	})
end

return tab
