-- The commands of the page, once: the id, the title, the group and the keys each has by default.
-- The palette, the help, the settings, the status line, the menus' keys and the host's list of
-- commands all come from this table, so a key is changed in one place: `keys` in the settings file
-- replaces the keys of a command by its id, and what cannot be taken (a key that is not a key, one the
-- terminal or the table has, one another command has) is refused with its reason in the settings.

local kit = require("../lib/kit")
local keys = require("@pencere/keys")

local S = kit.S

local commands = {}

-- The signals of the menu, in the order of how often they are wanted. `id` is the command.
commands.SIGNALS = {
	{ word = "TERM", id = "signal_term", title = "End it (TERM)", about = "asks it to end; most programs save their work and quit" },
	{ word = "INT", id = "signal_int", title = "Interrupt it (INT)", about = "does what Control-C does" },
	{ word = "HUP", id = "signal_hup", title = "Hang it up (HUP)", about = "tells it that its terminal is gone; many programs read their settings again" },
	{ word = "KILL", id = "signal_kill", title = "Kill it (KILL)", about = "ends it at once; it cannot save anything or clean up after itself" },
	{ word = "STOP", id = "signal_stop", title = "Stop it (STOP)", about = "freezes it until it is continued" },
	{ word = "CONT", id = "signal_cont", title = "Continue it (CONT)", about = "lets a stopped process run again" },
	{ word = "USR1", id = "signal_usr1", title = "Send USR1", about = "is a signal programs give a meaning of their own; one that has none ends" },
	{ word = "USR2", id = "signal_usr2", title = "Send USR2", about = "is a signal programs give a meaning of their own; one that has none ends" },
}

-- `when` names the condition a command can be done under (page.lua gives them: `commands.when`).
-- The keys are the few that are wanted often; the rest are in the menus and the palette. htop's
-- function keys are there too, as a second key where it has one.
local TABLE = {
	{ id = "pause", title = "Pause or resume updates", group = "Updates", keys = { "space" }, when = "idle" },
	{ id = "interval_faster", title = "Look at the machine more often", group = "Updates", keys = { "+", "=" }, when = "idle" },
	{ id = "interval_slower", title = "Look at the machine less often", group = "Updates", keys = { "-" }, when = "idle" },

	{ id = "filter", title = "Filter processes", group = "Table", keys = { "/", "f4" }, when = "idle" },
	{ id = "tree", title = "Show the process tree", group = "Table", keys = { "t", "f5" }, when = "idle" },
	{ id = "group", title = "Group processes by name", group = "Table", keys = { "g" }, when = "idle" },
	{ id = "fold", title = "Fold or open the branch", group = "Table", keys = { "f" }, when = "idle" },
	{ id = "fold_all", title = "Fold every branch", group = "Table", keys = { "[" }, when = "idle" },
	{ id = "open_all", title = "Open every branch", group = "Table", keys = { "]" }, when = "idle" },
	{ id = "sort_next", title = "Sort by the next column", group = "Table", keys = { ">", "f6" }, when = "idle" },
	{ id = "sort_prev", title = "Sort by the previous column", group = "Table", keys = { "<" }, when = "idle" },
	{ id = "reverse", title = "Reverse the order", group = "Table", keys = { "r" }, when = "idle" },
	{ id = "mark", title = "Mark or unmark the process", group = "Table", keys = { "m" }, when = "idle" },
	{ id = "subtree", title = "Only this process and its children", group = "Table", keys = { "o" }, when = "idle" },
	{ id = "filter_user", title = "Show only the processes of this user", group = "Table", when = "process" },
	{ id = "filter_name", title = "Show only the processes of this name", group = "Table", when = "process" },

	{ id = "card_cpu", title = "Show the details of CPU", group = "Cards", keys = { "1" }, when = "idle" },
	{ id = "card_memory", title = "Show the details of Memory", group = "Cards", keys = { "2" }, when = "idle" },
	{ id = "card_network", title = "Show the details of Network", group = "Cards", keys = { "3" }, when = "idle" },
	{ id = "card_disk", title = "Show the details of Disk", group = "Cards", keys = { "4" }, when = "idle" },
	{ id = "card_gpu", title = "Show the details of GPU", group = "Cards", keys = { "5" }, when = "gpu" },
	{ id = "range_next", title = "Change how long the graphs look back", group = "Cards", keys = { "w" }, when = "idle" },

	{ id = "ports", title = "Show or hide the listening ports", group = "Ports", keys = { "p" }, when = "idle" },
	{ id = "port_show", title = "Show the process of the socket in the table", group = "Ports", when = "ports" },
	{ id = "copy_address", title = "Copy the address of the socket", group = "Ports", keys = { "a" }, when = "ports" },
	{ id = "copy_port", title = "Copy the port of the socket", group = "Ports", when = "ports" },

	{ id = "detail", title = "Show or hide the details of the process", group = "Process", keys = { "i" }, when = "process" },
	{ id = "row_menu", title = "Open the menu of the process", group = "Process", keys = { "." }, when = "process" },
	{ id = "signal", title = "Send a signal to the process…", group = "Process", keys = { "k", "f9" }, when = "process" },
	{ id = "renice", title = "Change the priority of the process…", group = "Process", keys = { "n", "f7" }, when = "process" },
	{ id = "shell", title = "Open a shell in the folder of the process", group = "Process", keys = { "s" }, when = "process" },
	{ id = "copy_command", title = "Copy the command line", group = "Process", keys = { "c" }, when = "process" },
	{ id = "copy_pid", title = "Copy the process id", group = "Process", when = "process" },
	{ id = "copy_cwd", title = "Copy the working folder", group = "Process", when = "process" },
	-- The signals are here, after the others of the process.

	{ id = "palette", title = "Find a command or a process", group = "Toppen", keys = { "cmd+k", "ctrl+k" }, when = "idle" },
	{ id = "help", title = "Show the keys", group = "Toppen", keys = { "?", "f1" }, when = "idle" },
	{ id = "settings", title = "Open the settings", group = "Toppen", keys = { "cmd+,", "f2" }, when = "idle" },
}
for _, signal in commands.SIGNALS do
	TABLE[#TABLE + 1] = { id = signal.id, title = signal.title .. "…", group = "Process", when = "process" }
end

-- The conditions, by name; page.lua fills them in (they depend on parts that need this file).
commands.when = {}

-- What a key is, for comparing: modifiers in one order, then the key as the kit names it.
local function canon(parsed)
	local mods = parsed.mods
	local out = {}
	for _, name in { "control", "alt", "shift", "command" } do
		if mods[name] then
			out[#out + 1] = name
		end
	end
	out[#out + 1] = parsed.key
	return table.concat(out, "+")
end

-- The keys the table, the fields and the dialogs have for themselves, and the terminal's own (the
-- keys of bateri that a pane never gets: ⌘P ⌘W ⌘O ⌘T ⌘N ⌘D ⌘[ ⌘] ⌘1–9, ⌥⌘ and the arrows, ⌃Space).
local TAKEN = {}
for _, name in { "up", "down", "left", "right", "enter", "escape", "tab", "home", "end", "page_up", "page_down", "backspace", "delete" } do
	TAKEN[name] = "is used by the table, the fields and the dialogs"
end
for _, words in { "cmd+p", "cmd+w", "cmd+o", "cmd+t", "cmd+n", "cmd+d", "cmd+[", "cmd+]", "ctrl+space", "alt+cmd+up", "alt+cmd+down", "alt+cmd+left", "alt+cmd+right" } do
	local parsed = keys.parse(words)
	TAKEN[canon(parsed)] = "is the terminal's own"
end
for digit = 1, 9 do
	TAKEN[canon(keys.parse("cmd+" .. digit))] = "is the terminal's own"
end

-- Why the keys `words` cannot be given to a command, or nil where they can. `parsed` is `keys.parse`.
local function refused(words)
	local parsed, why = keys.parse(words)
	if parsed == nil then
		return nil, why or "is not a key"
	end
	local taken = TAKEN[canon(parsed)]
	if taken ~= nil then
		return nil, taken
	end
	-- ⌥ and a letter is how the terminal writes what the keyboard cannot (accents): not bound.
	if parsed.mods.alt and not parsed.mods.command and #parsed.key == 1 and string.match(parsed.key, "%a") then
		return nil, "is Option and a letter, which a keyboard writes accents with"
	end
	return canon(parsed)
end

-- The commands with the keys they have now: `overrides` is the settings' `keys` (id → list). Returns
-- the list, `{ id, title, group, keys, when, default }` each, and the problems: `{ id, words, why }`
-- for each key that was asked for and not given. A command without a setting has its own keys; one
-- whose keys are all refused has its own too, where they are free.
function commands.resolve(overrides)
	overrides = overrides or {}
	local claimed, effective, problems = {}, {}, {}
	local titles = {}
	for _, command in TABLE do
		titles[command.id] = command.title
	end
	local function claim(id, words)
		local canonical = refused(words)
		if canonical ~= nil and (claimed[canonical] == nil or claimed[canonical] == id) then
			claimed[canonical] = id
			return true
		end
		return false
	end
	for _, command in TABLE do
		if overrides[command.id] == nil then
			effective[command.id] = {}
			for _, words in command.keys or {} do
				if claim(command.id, words) then
					effective[command.id][#effective[command.id] + 1] = words
				else
					-- A key of this table that is not free: a mistake of the table, said where it shows.
					problems[#problems + 1] = { id = command.id, title = command.title, words = words, why = "is not free (a mistake in the table of commands)" }
				end
			end
		end
	end
	for _, command in TABLE do
		local asked = overrides[command.id]
		if type(asked) == "table" then
			local given = {}
			for _, words in asked do
				local canonical, why = refused(words)
				if canonical == nil then
					problems[#problems + 1] = { id = command.id, title = command.title, words = words, why = why }
				elseif claimed[canonical] ~= nil and claimed[canonical] ~= command.id then
					problems[#problems + 1] = {
						id = command.id,
						title = command.title,
						words = words,
						why = "is used by " .. titles[claimed[canonical]],
					}
				else
					claimed[canonical] = command.id
					given[#given + 1] = words
				end
			end
			if #given == 0 and #asked > 0 then
				for _, words in command.keys or {} do
					if claim(command.id, words) then
						given[#given + 1] = words
					end
				end
			end
			effective[command.id] = given
		end
	end
	local list = {}
	for _, command in TABLE do
		list[#list + 1] = {
			id = command.id,
			title = command.title,
			group = command.group,
			keys = effective[command.id],
			when = command.when,
			default = command.keys or {},
		}
	end
	return list, problems
end

-- --- What is in force --------------------------------------------------------------------------------

local current = nil
local current_problems = {}

-- Reads the settings' keys again (at the start, and when the person changes one).
function commands.reload()
	local settings = S.boot and S.boot.settings and S.boot.settings.values or {}
	current, current_problems = commands.resolve(settings.keys)
end

function commands.problems()
	return current_problems
end

-- The commands with their keys now.
function commands.all()
	if current == nil then
		commands.reload()
	end
	return current
end

function commands.get(id)
	for _, command in commands.all() do
		if command.id == id then
			return command
		end
	end
	return nil
end

-- The first key of command `id`, as it is written on this keyboard; nil where it has none.
function commands.label(id)
	local command = commands.get(id)
	if command == nil or #command.keys == 0 then
		return nil
	end
	return keys.label(command.keys[1])
end

-- The words of the first key of `id` (what the kit's `keys =` takes), or nil.
function commands.first(id)
	local command = commands.get(id)
	return command ~= nil and command.keys[1] or nil
end

-- The list for the library's `ui.commands`: each with its `when` made a function.
function commands.for_library()
	local out = {}
	for _, command in commands.all() do
		local name = command.when
		out[#out + 1] = {
			id = command.id,
			title = command.title,
			group = command.group,
			keys = #command.keys > 0 and command.keys or nil,
			when = function()
				local when = commands.when[name]
				return when == nil or when()
			end,
		}
	end
	return out
end

-- Whether `words` is something a command can be given (the settings page asks before it writes).
function commands.check(id, words)
	local canonical, why = refused(words)
	if canonical == nil then
		return false, why
	end
	for _, command in commands.all() do
		if command.id ~= id then
			for _, held in command.keys do
				local other = keys.parse(held)
				if other ~= nil and canon(other) == canonical then
					return false, "is used by " .. command.title
				end
			end
		end
	end
	return true
end

return commands
