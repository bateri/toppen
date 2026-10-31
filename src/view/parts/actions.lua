-- What is done to a process: a signal (asked of the person first, in the page, with the whole command
-- line before them), a new priority, a shell in its folder, a copy of what names it, a filter made of
-- its user or its name. Each is a command with an id (the keys, the menus and the buttons of the
-- panel all come here), and the menus, the dialogs and the toasts of them are drawn here.
--
-- The program does the doing: it reads the process's start again right before it signals or
-- renices, and says `gone` if it is not the one that was named (nothing is sent then).

local kit = require("../lib/kit")
local tab = require("./table")
local filter = require("./filter")
local detail = require("./detail")
local commands = require("./commands")

local b, u, S, act, lib = kit.b, kit.u, kit.S, kit.act, kit.lib

local actions = {}

-- What the person is asked now: nil, or { kind = "signal", target =, word =, ... } /
-- { kind = "renice", target =, error = }.
local R = { dialog = nil }

S.toasts = {}

local SIGNALS = commands.SIGNALS
local BY_ID, BY_WORD = {}, {}
for _, signal in SIGNALS do
	BY_ID[signal.id] = signal
	BY_WORD[signal.word] = signal
end

-- --- Toasts ----------------------------------------------------------------------------------------------------

local function toast(words, tone, seconds)
	lib.toast(S.toasts, words, tone, seconds)
end

actions.toast = toast

-- --- Asking ----------------------------------------------------------------------------------------------------

-- The process a command is about, or nil with a word to the person.
local function target_of(needs_rights)
	local target = detail.target()
	if target == nil then
		toast("Select a process first.", "warn")
		return nil
	end
	if needs_rights and not target.mine then
		toast(string.format("%s belongs to %s: that takes root.", target.name, kit.user_shown(target.user)), "warn")
		return nil
	end
	if needs_rights and target.ended then
		toast(string.format("%s has ended.", target.name), "warn")
		return nil
	end
	return target
end

local function ask_signal(word)
	local target = target_of(true)
	if target ~= nil then
		R.dialog = { kind = "signal", target = target, word = word }
	end
	return true
end

local function ask_renice()
	local target = target_of(true)
	if target ~= nil then
		R.dialog = { kind = "renice", target = target, error = nil }
	end
	return true
end

local function signal_menu(near)
	local target = target_of(true)
	if target == nil then
		return true
	end
	local entries = {}
	for _, signal in SIGNALS do
		entries[#entries + 1] = { key = signal.id, title = signal.title }
	end
	if near ~= nil then
		return u:menu("signal-menu", entries, { near = near })
	end
	return u:menu("signal-menu", entries, { at = tab.anchor() })
end

-- --- Doing -----------------------------------------------------------------------------------------------------

local function who(target)
	return string.format("%s (%d)", target.name, target.pid)
end

local function report_signal(result, target, word, tree)
	local outcome = result.outcome
	if outcome == "done" then
		if tree and result.done > 1 then
			toast(string.format("Sent %s to %s and the %d below it.", word, who(target), result.done - 1), "ok")
		else
			toast(string.format("Sent %s to %s.", word, who(target)), "ok")
		end
	elseif outcome == "gone" then
		toast(string.format("%s had already ended. Nothing was sent.", who(target)), "warn")
	elseif outcome == "denied" then
		toast(string.format("The system did not allow it: %s belongs to %s.", who(target), kit.user_shown(target.user)), "bad")
	else
		toast(string.format("%s could not be signalled: %s", who(target), result.message or "the system said no."), "bad")
	end
end

local function send_signal(target, word, tree)
	kit.call("signal", { pid = target.pid, start = target.start, signal = word, tree = tree }, function(result, err)
		if err then
			toast(err.message or err.code, "bad")
		else
			report_signal(result, target, word, tree)
		end
		return true
	end)
end

local function set_nice(target, nice)
	kit.call("renice", { pid = target.pid, start = target.start, nice = nice }, function(result, err)
		if err then
			toast(err.message or err.code, "bad")
		elseif result.outcome == "done" then
			toast(string.format("The priority of %s is now %d.", who(target), nice), "ok")
		elseif result.outcome == "gone" then
			toast(string.format("%s had already ended. Nothing was changed.", who(target)), "warn")
		elseif result.outcome == "denied" then
			if nice < target.nice then
				toast("A priority higher than it has takes root.", "bad")
			else
				toast(string.format("The system did not allow it: %s belongs to %s.", who(target), kit.user_shown(target.user)), "bad")
			end
		else
			toast(string.format("The priority could not be changed: %s", result.message or "the system said no."), "bad")
		end
		return true
	end)
end

local function open_shell()
	local target = target_of(false)
	if target == nil then
		return true
	end
	detail.with_info(target, function(info)
		kit.call("shell", { cwd = info and info.cwd or nil }, function(_, err)
			if err then
				toast(err.message or err.code, "bad")
			end
			return true
		end)
	end)
	return true
end

local function copy(words, what)
	kit.gather({ copy = words })
	toast("Copied " .. what .. ".", "ok", 2)
end

local function copy_command()
	local target = target_of(false)
	if target == nil then
		return true
	end
	-- The row's command line is cut; the whole of it is what the details have.
	detail.with_info(target, function(info)
		copy(info and table.concat(info.argv, " ") or target.command, "the command line")
	end)
	return true
end

local function copy_cwd()
	local target = target_of(false)
	if target == nil then
		return true
	end
	detail.with_info(target, function(info)
		if info and info.cwd then
			copy(info.cwd, "the working folder")
		else
			toast("The system does not tell where this process works.", "warn")
		end
	end)
	return true
end

local function filter_term(what)
	local target = target_of(false)
	if target == nil then
		return true
	end
	if what == "user" then
		filter.add("user:" .. target.user)
	elseif string.find(target.name, "[%s\"]") then
		filter.add('"' .. string.gsub(target.name, '"', "") .. '"')
	else
		filter.add(target.name)
	end
	return true
end

-- --- Commands --------------------------------------------------------------------------------------------------

-- Does the command `id` of this file; nil where it is not one of them. `near` is the element a menu
-- opens by (the button that asked).
function actions.command(id, near)
	local signal = BY_ID[id]
	if signal ~= nil then
		return ask_signal(signal.word)
	elseif id == "signal" then
		return signal_menu(near)
	elseif id == "renice" then
		return ask_renice()
	elseif id == "shell" then
		return open_shell()
	elseif id == "copy_command" then
		return copy_command()
	elseif id == "copy_pid" then
		local target = target_of(false)
		if target ~= nil then
			copy(tostring(target.pid), "the process id")
		end
		return true
	elseif id == "copy_cwd" then
		return copy_cwd()
	elseif id == "filter_user" then
		return filter_term("user")
	elseif id == "filter_name" then
		return filter_term("name")
	elseif id == "detail" then
		detail.toggle()
		return true
	elseif id == "row_menu" then
		local row = tab.current()
		if row ~= nil then
			return actions.row_menu(row, tab.anchor())
		end
		return true
	end
	return nil
end

-- --- The menu of a row ---------------------------------------------------------------------------------

function actions.row_menu(row, at)
	local person = S.boot.person
	local mine = person.root or person.name == row.user
	local entries = {}
	local function add(key, title, keys, enabled)
		entries[#entries + 1] = { key = key, title = title, keys = keys, enabled = enabled ~= false }
	end
	local shown = detail.state.open and detail.state.key ~= nil and detail.state.key.pid == row.pid and detail.state.key.start == row.start
	add("detail", shown and "Hide the details" or "Show the details", commands.first("detail"))
	entries[#entries + 1] = "-"
	add("signal_term", "End it (TERM)", nil, mine)
	add("signal_kill", "Kill it (KILL)", nil, mine)
	add("signal_stop", "Stop it (STOP)", nil, mine)
	add("signal_cont", "Continue it (CONT)", nil, mine)
	add("signal", "More signals…", commands.first("signal"), mine)
	add("renice", "Change the priority…", commands.first("renice"), mine)
	entries[#entries + 1] = "-"
	add("copy_command", "Copy the command line", commands.first("copy_command"))
	add("copy_pid", "Copy the process id")
	add("copy_cwd", "Copy the working folder")
	entries[#entries + 1] = "-"
	add("filter_user", "Show only " .. kit.user_shown(row.user), nil)
	add("filter_name", "Show only " .. row.name, nil)
	add("subtree", tab.state.view.root ~= nil and "Show every process" or "Only this process and its children", commands.first("subtree"))
	add("mark", S.marks[row.pid] and "Unmark" or "Mark", commands.first("mark"))
	if row.children > 0 then
		add("fold", (row.folded and "Open" or "Fold") .. " the branch", commands.first("fold"))
	end
	entries[#entries + 1] = "-"
	add("shell", "Open a shell in its folder", commands.first("shell"))
	return u:menu("row-menu", entries, { at = at })
end

act.row_menu = actions.row_menu

-- --- Drawing the dialogs and the toasts ------------------------------------------------------------

local function signal_dialog(d)
	local target, word = d.target, d.word
	local signal = BY_WORD[word]
	local title = word == "KILL" and string.format("Kill %s?", target.name) or string.format("Send %s to %s?", word, target.name)
	local message = string.format("Process %d of %s:\n%s\n\n%s %s.", target.pid, kit.user_shown(target.user), target.command, word, signal.about)
	local list = { { key = "cancel", title = "Cancel" } }
	if target.children == nil or target.children > 0 then
		local below = target.children and string.format("the %d below it", target.children) or "everything below it"
		list[#list + 1] = { key = "tree", title = "With " .. below, danger = word == "KILL" }
	end
	list[#list + 1] = { key = "send", title = word == "KILL" and "Kill" or ("Send " .. word), primary = true, danger = word == "KILL" }
	return u:dialog("confirm", { title = title, message = message, actions = list, width = 480 })
end

local function renice_dialog(d)
	local target = d.target
	local message = "From -20 (first in line) to 19 (last). Raising the number is free; lowering it takes root."
	if d.error then
		message = d.error
	end
	return u:dialog("renice", {
		title = string.format("Priority of %s", target.name),
		message = message,
		input = { value = tostring(target.nice), label = "Nice value", hint = "-20 to 19", select = true },
		actions = { { key = "cancel", title = "Cancel" }, { key = "set", title = "Set", primary = true } },
		width = 420,
	})
end

function actions.view(root)
	lib.toasts_prune(S.toasts, S.now)
	if R.dialog ~= nil then
		root:child(R.dialog.kind == "signal" and signal_dialog(R.dialog) or renice_dialog(R.dialog))
	end
	if #S.toasts > 0 then
		root:child(u:toasts("toasts", { items = S.toasts }))
	end
end

-- --- Answers -----------------------------------------------------------------------------------------------------

local function answered_signal(answer)
	local d = R.dialog
	R.dialog = nil
	if answer.key == "send" then
		send_signal(d.target, d.word, false)
	elseif answer.key == "tree" then
		send_signal(d.target, d.word, true)
	end
end

local function answered_renice(answer)
	local d = R.dialog
	if answer.key ~= "set" then
		R.dialog = nil
		return
	end
	local nice = tonumber(answer.value)
	if nice == nil or nice ~= math.floor(nice) or nice < -20 or nice > 19 then
		d.error = "A priority is a whole number from -20 to 19."
		return
	end
	R.dialog = nil
	set_nice(d.target, nice)
end

-- An action of a dialog or a toast of this file. Whether it was one of them, and its reply.
function actions.action(action)
	local from, kind = action.from, action.kind
	if from == "confirm" and kind == "answer" then
		answered_signal(action)
		return true
	elseif from == "renice" and kind == "answer" then
		answered_renice(action)
		return true
	elseif from == "toasts" and kind == "close" then
		for index, item in S.toasts do
			if item.key == action.key then
				table.remove(S.toasts, index)
				break
			end
		end
		return true
	end
	return nil
end

return actions
