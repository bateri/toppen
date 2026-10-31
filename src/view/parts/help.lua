-- The help: every command with the keys it has now (the settings' keys, not only the defaults), in
-- groups, from the same table as the palette and the status line. A modal over the page; Esc closes it.

local kit = require("../lib/kit")
local commands = require("./commands")

local b, u, S, act = kit.b, kit.u, kit.S, kit.act

local help = {}

local H = { open = false }
help.state = H

-- What the table, the fields and the dialogs have for themselves: not commands, and not to be changed.
local MOVES = {
	{ keys = { "up", "down" }, title = "Move through the rows" },
	{ keys = { "page_up", "page_down" }, title = "Move a page at a time" },
	{ keys = { "home", "end" }, title = "Go to the first or the last row" },
	{ keys = { "enter" }, title = "Open the details of the row" },
	{ keys = { "escape" }, title = "Go back, close what is open, give up a question" },
	{ keys = { "tab" }, title = "Move the keyboard to the next part of the page" },
}

function help.show()
	H.open = true
end

function help.hide()
	H.open = false
end

local function label(words)
	return b.text(words):text_size(11):color("quiet"):weight(600)
end

local function keys_of(list, limit)
	local row = b.div():row():items_center():gap(6):no_shrink()
	if #list == 0 then
		return row:child(b.text("no key"):text_size(12):color("quiet"))
	end
	for index, words in list do
		if index > limit then
			break
		end
		row:child(u:kbd(words, { quiet = true }))
	end
	return row
end

local function line(title, list, w)
	local row = b.div():row():items_center():gap(10):h(24):w(w):no_shrink()
	row:child(b.text(title):truncate():min_w(0):grow(1))
	row:child(keys_of(list, 3))
	return row
end

function help.view(root)
	if not H.open then
		return
	end
	local width = math.min(680, S.width - 32)
	local inner = width - 32
	local room = math.max(S.height - 140, 160)
	local body = b.div():col():gap(10):p(16)
	local head = b.div():row():items_center():gap(10):w(inner)
	head:child(b.text("Keys and commands"):text_size(16):weight(650))
	head:child(b.spacer())
	head:child(u:icon_button("help-close", { icon = "close", label = "Close the keys" }))
	body:child(head)
	local settings = commands.label("settings")
	local palette = commands.label("palette")
	body:child(b.text(string.format(
		"%s finds any command and any process by name%s. Each key can be changed in the settings%s.",
		palette or "The palette",
		palette and "" or " (it has no key now)",
		settings and (" (" .. settings .. ")") or ""
	)):color("dim"):text_size(12):wrap():w(inner))

	local list = b.div():col():gap(2):w(inner):no_shrink()
	list:child(label("Movement"))
	for _, move in MOVES do
		list:child(line(move.title, move.keys, inner))
	end
	local seen, order = {}, {}
	for _, command in commands.all() do
		local group = seen[command.group]
		if group == nil then
			group = {}
			seen[command.group] = group
			order[#order + 1] = command.group
		end
		group[#group + 1] = command
	end
	for _, name in order do
		list:child(b.div():h(6):no_shrink())
		list:child(label(name))
		for _, command in seen[name] do
			list:child(line(command.title, command.keys, inner))
		end
	end
	body:child(b.div():id("help/scroll"):scroll():max_h(room):w(inner):child(list))
	root:child(u:modal("help", { width = width, label = "the keys", top = 40 }, body))
end

function help.action(action)
	if action.from == "help" and action.kind == "dismiss" then
		H.open = false
		return true
	elseif action.from == "help-close" and action.kind == "press" then
		H.open = false
		return true
	end
	return nil
end

act.open_help = function()
	H.open = true
end

return help
