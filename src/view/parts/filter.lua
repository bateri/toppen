-- The bar over the table: the filter field, how many processes it lets through, the shape of the
-- table (flat, tree or groups), and the two things that can be taken off it: a held order and a
-- narrowing to one process's subtree.

local kit = require("../lib/kit")
local tab = require("./table")

local b, u, S = kit.b, kit.u, kit.S

local filter = {}

local PAD = 10
local BAR = 34

-- The field's state: what it holds.
local F = { value = "" }

local MODES = {
	{ key = "flat", title = "Flat" },
	{ key = "tree", title = "Tree" },
	{ key = "group", title = "Group" },
}

-- Puts the keyboard in the field, for `/`.
function filter.focus()
	return u:focus(u:name("filter", "field"))
end

-- Puts the keyboard back in the table.
function filter.leave()
	return u:focus(u:name("procs", "body"))
end

local function set_text(text)
	F.value = text
	kit.call("view", { filter = text }, function(result)
		if result then
			tab.state.view = result
		end
		return true
	end)
end

-- Adds `term` to what the field holds (once), as if it was typed.
function filter.add(term)
	if string.find(" " .. F.value .. " ", " " .. term .. " ", 1, true) ~= nil then
		return
	end
	set_text(F.value == "" and term or (F.value .. " " .. term))
end

-- An action of the bar. Whether it was the bar's, and its reply.
function filter.action(action)
	local from, kind = action.from, action.kind
	if from == "filter" then
		if kind == "change" then
			set_text(action.text)
		elseif kind == "submit" then
			return filter.leave()
		elseif kind == "escape" then
			-- The first escape clears the field, the next leaves it.
			if F.value ~= "" then
				set_text("")
			else
				return filter.leave()
			end
		end
		return true
	elseif from == "filter-clear" and kind == "press" then
		set_text("")
		return filter.focus()
	elseif from == "mode" and kind == "select" then
		tab.set_mode(action.key)
		return true
	elseif from == "release" and kind == "press" then
		tab.release()
		return true
	elseif from == "subtree" and kind == "press" then
		tab.leave_subtree()
		return true
	end
	return nil
end

-- Words for how many processes there are.
local function count_words(T)
	if T.view.filter ~= "" and T.view.error == nil then
		return string.format("%d of %d processes", T.matched, T.total)
	end
	return string.format("%d processes", T.total)
end

function filter.view()
	local T = tab.summary()
	local view = T.view
	local narrow = S.width < 560
	local trail = {}
	if F.value ~= "" then
		trail[1] = u:icon_button("filter-clear", { icon = "close", label = "Clear the filter", keys = "escape" })
	end
	local field = u:input("filter", {
		state = F,
		label = "Filter processes",
		hint = "Filter: name, /regex/, user:, cpu>50  ·  /",
		lead = kit.icon("search", 14, "quiet"),
		trail = trail,
		escape = true,
		tone = view.error and "bad" or nil,
	})
	local bar = b.div():row():items_center():gap(8):px(PAD):h(BAR):no_shrink()
	bar:child(b.div():grow(1):min_w(0):max_w(480):child(field))
	bar:child(b.text(count_words(T)):color("dim"):no_shrink())
	if view.root ~= nil then
		bar:child(u:button("subtree", { title = "Only " .. (T.root_name or "one process"), icon = "close" }))
	end
	if T.held and T.moved > 0 then
		bar:child(u:button("release", { title = string.format("Order held · %d moved", T.moved) }))
	end
	bar:child(b.spacer())
	if not narrow then
		bar:child(u:segments("mode", { value = view.mode, items = MODES }))
	end
	local box = b.div():col():no_shrink()
	box:child(bar)
	if view.error ~= nil then
		box:child(b.div():px(PAD):pb(4):child(b.text(view.error):color("error"):text_size(11.5):truncate():min_w(0)))
	end
	return box
end

return filter
