-- The settings: how often the machine is looked at, whether the names are shown, how the table
-- starts, and the keys of every command. Kept by the program in the person's file (`settings`); this
-- is the page that changes them. A key is changed by pressing it: the next key is the command's.

local kit = require("../lib/kit")
local keys = require("@pencere/keys")
local commands = require("./commands")
local tab = require("./table")
local actions = require("./actions")

local b, u, S, act = kit.b, kit.u, kit.S, kit.act

local settings = {}

-- What is open, which command is waiting for its key, and what was last said about a key.
local Z = { open = false, capture = nil, note = nil }
settings.state = Z

local INTERVALS = { 0.5, 1, 2, 5, 10 }
local MODIFIERS = { control = true, ctrl = true, alt = true, shift = true, command = true, meta = true, super = true }

function settings.show()
	Z.open = true
	Z.capture, Z.note = nil, nil
end

function settings.hide()
	Z.open = false
	Z.capture, Z.note = nil, nil
end

function settings.capturing()
	return Z.open and Z.capture ~= nil
end

-- --- What is in force ---------------------------------------------------------------------------------

local function values()
	return S.boot and S.boot.settings and S.boot.settings.values or {}
end

-- What the settings change in the page itself, once they are known and when they change.
local function apply(values_)
	S.private = values_.privacy == true
	commands.reload()
	if act.keys_changed ~= nil then
		act.keys_changed()
	end
end

-- The settings, at the start: the table and the panel as they were left, the names, the keys.
function settings.start(boot)
	local held = boot.settings.values or {}
	apply(held)
	tab.apply_settings(held)
	S.details_at_start = held.details == true
	if boot.settings.error ~= nil then
		actions.toast("The settings file could not be used, so the defaults are in force. The settings say why.", "warn", 12)
	end
	local problems = commands.problems()
	if #problems > 0 then
		actions.toast(string.format("%d of the keys in the settings are not used: see the settings.", #problems), "warn", 8)
	end
end

-- Changes settings: `set` is what `settings` takes. Said to the person unless `quiet`.
local function save(set, quiet)
	kit.call("settings", { set = set }, function(result, err)
		if result then
			S.boot.settings = result
			apply(result.values or {})
		elseif not quiet then
			actions.toast(err and err.message or "The settings could not be written.", "bad", 8)
		end
		return true
	end)
end

act.save_setting = save

-- --- The keys of an event, as the settings write them ----------------------------------------------------------

-- The words of the key event `event` ("ctrl+k", "f4", "?"); nil for a modifier on its own.
local function words_of(event)
	local name = event.key
	if name == nil or MODIFIERS[string.lower(name)] then
		return nil
	end
	local mods = event.mods or {}
	local parts = {}
	if mods.control then
		parts[#parts + 1] = "ctrl"
	end
	if mods.alt then
		parts[#parts + 1] = "alt"
	end
	local letter = #name == 1 and string.match(name, "%a") ~= nil
	-- The shift of a character that is not a letter is in the character (">" is shift and "."), as
	-- the library's commands match it.
	if mods.shift and (letter or #name > 1) then
		parts[#parts + 1] = "shift"
	end
	if mods.command then
		parts[#parts + 1] = "cmd"
	end
	parts[#parts + 1] = name == " " and "space" or (letter and string.lower(name) or name)
	return table.concat(parts, "+")
end

-- A key pressed while a command waits for one. Its reply.
function settings.capture(event)
	local id = Z.capture
	if event.type ~= "key" then
		return true
	end
	local words = words_of(event)
	if words == nil then
		return true
	elseif words == "escape" then
		Z.capture = nil
		Z.note = nil
		return true
	end
	local held = {}
	for command_id, list in values().keys or {} do
		held[command_id] = list
	end
	if words == "backspace" then
		held[id] = {}
	else
		local ok, why = commands.check(id, words)
		if not ok then
			Z.note = string.format("%s %s.", keys.label(words), why)
			return true
		end
		held[id] = { words }
	end
	Z.capture = nil
	Z.note = nil
	save({ keys = next(held) ~= nil and held or b.json.null })
	return true
end

-- --- Drawing ---------------------------------------------------------------------------------------------

local function label(words)
	return b.text(words):text_size(11):color("quiet"):weight(600)
end

local function key_of(seconds)
	return tostring(seconds)
end

local function row(inner, name, control)
	local line = b.div():row():items_center():gap(10):w(inner):no_shrink()
	line:child(b.text(name):truncate():min_w(0):grow(1))
	line:child(control)
	return line
end

local function table_words()
	local now = tab.snapshot()
	local column = "CPU"
	for _, one in tab.columns() do
		if one.key == now.sort then
			column = one.title
		end
	end
	return string.format(
		"%s, %s, %s; %d columns hidden",
		column,
		now.descending and "largest first" or "smallest first",
		now.mode == "flat" and "as a list" or ("as a " .. (now.mode == "tree" and "tree" or "set of groups")),
		#now.hidden
	)
end

local function key_row(command, inner)
	local waiting = Z.capture == command.id
	local line = b.div():row():items_center():gap(8):h(30):w(inner):no_shrink()
	line:child(b.text(command.title):truncate():min_w(0):grow(1))
	if waiting then
		line:child(b.text("Press the new key"):color("accent"):weight(600):no_shrink())
		line:child(u:button("key-cancel", { title = "Cancel", keys = "escape" }))
		return line
	end
	if #command.keys == 0 then
		line:child(b.text("no key"):color("quiet"):text_size(12):no_shrink())
	else
		for index, words in command.keys do
			if index <= 3 then
				line:child(u:kbd(words, { quiet = true }))
			end
		end
	end
	line:child(u:button("key-change/" .. command.id, { title = "Change" }))
	local changed = (values().keys or {})[command.id] ~= nil
	if changed then
		line:child(u:button("key-reset/" .. command.id, { title = "Reset" }))
	end
	return line
end

function settings.view(root)
	if not Z.open then
		return
	end
	local width = math.min(680, S.width - 32)
	local inner = width - 32
	local held = values()
	local body = b.div():col():gap(12):p(16)
	local head = b.div():row():items_center():gap(10):w(inner)
	head:child(b.text("Settings"):text_size(16):weight(650))
	head:child(b.spacer())
	head:child(u:icon_button("settings-close", { icon = "close", label = "Close the settings" }))
	body:child(head)

	local scroll = b.div():col():gap(12):w(inner):no_shrink()
	local path = S.boot.settings.path
	scroll:child(b.text(path ~= nil and ("Kept in " .. path) or "Kept for this run only"):color("quiet"):text_size(12):truncate():min_w(0):w(inner))
	if S.boot.settings.error ~= nil then
		scroll:child(b.text(S.boot.settings.error .. ". Nothing is written until it is fixed."):color("warning"):text_size(12):wrap():w(inner))
	end

	scroll:child(label("Refresh"))
	local steps = {}
	for _, seconds in INTERVALS do
		steps[#steps + 1] = { key = key_of(seconds), title = seconds .. " s" }
	end
	scroll:child(row(inner, "Look at the machine every", u:segments("set-interval", { value = key_of(S.interval), items = steps })))

	scroll:child(label("Privacy"))
	scroll:child(row(inner, "The machine's name and mine", u:segments("set-privacy", {
		value = S.private and "hidden" or "shown",
		items = { { key = "shown", title = "Shown" }, { key = "hidden", title = "Hidden" } },
	})))

	scroll:child(label("Table"))
	scroll:child(row(inner, "Open the panel of a process at the start", u:segments("set-details", {
		value = held.details == true and "open" or "closed",
		items = { { key = "closed", title = "Closed" }, { key = "open", title = "Open" } },
	})))
	scroll:child(b.text("Now: " .. table_words()):color("dim"):text_size(12):truncate():min_w(0):w(inner))
	local buttons = b.div():row():gap(8):no_shrink()
	buttons:child(u:button("set-table-save", { title = "Start the table like this" }))
	buttons:child(u:button("set-table-reset", { title = "Back to the defaults" }))
	scroll:child(buttons)

	scroll:child(label("Keys"))
	local problems = commands.problems()
	for _, problem in problems do
		scroll:child(b.text(string.format("Not used: “%s” for %s %s.", problem.words, problem.title, problem.why))
			:color("warning"):text_size(12):wrap():w(inner))
	end
	if Z.note ~= nil then
		scroll:child(b.text(Z.note):color("warning"):text_size(12):wrap():w(inner))
	end
	scroll:child(b.text("Press Change, then the key. Backspace takes a command's keys away."):color("dim"):text_size(12):wrap():w(inner))
	local group = nil
	for _, command in commands.all() do
		if command.group ~= group then
			group = command.group
			scroll:child(b.div():h(4):no_shrink())
			scroll:child(label(group))
		end
		scroll:child(key_row(command, inner))
	end
	body:child(b.div():id("settings/scroll"):scroll():max_h(math.max(S.height - 140, 160)):w(inner):child(scroll))
	root:child(u:modal("settings", { width = width, label = "the settings", top = 40 }, body))
end

-- --- What the person does -----------------------------------------------------------------------------------

function settings.action(action)
	local from, kind = action.from, action.kind
	if from == "settings" and kind == "dismiss" then
		if Z.capture ~= nil then
			Z.capture = nil
			return true
		end
		settings.hide()
		return true
	elseif from == "settings-close" and kind == "press" then
		settings.hide()
		return true
	elseif from == "set-interval" and kind == "select" then
		local seconds = tonumber(action.key)
		if seconds ~= nil then
			kit.call("sampling", { interval = seconds }, function(result)
				if result then
					S.interval, S.paused = result.interval, result.paused
					save({ interval = result.interval }, true)
				end
				return true
			end)
		end
		return true
	elseif from == "set-privacy" and kind == "select" then
		save({ privacy = action.key == "hidden" })
		return true
	elseif from == "set-details" and kind == "select" then
		save({ details = action.key == "open" })
		return true
	elseif from == "set-table-save" and kind == "press" then
		local now = tab.snapshot()
		save({ sort = now.sort, descending = now.descending, mode = now.mode, hidden = now.hidden })
		return true
	elseif from == "set-table-reset" and kind == "press" then
		save({ sort = b.json.null, descending = b.json.null, mode = b.json.null, hidden = b.json.null, details = b.json.null })
		return true
	elseif from == "key-cancel" and kind == "press" then
		Z.capture, Z.note = nil, nil
		return true
	elseif kind == "press" and type(from) == "string" then
		local change = string.match(from, "^key%-change/(.+)$")
		if change ~= nil then
			Z.capture, Z.note = change, nil
			return true
		end
		local reset = string.match(from, "^key%-reset/(.+)$")
		if reset ~= nil then
			local held = {}
			for command_id, list in values().keys or {} do
				if command_id ~= reset then
					held[command_id] = list
				end
			end
			Z.note = nil
			save({ keys = next(held) ~= nil and held or b.json.null })
			return true
		end
	end
	return nil
end

act.open_settings = settings.show

return settings
