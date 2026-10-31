-- The palette: a field over every command that can be done now (with its keys), and what is typed
-- as a name or a port: "Filter the processes by …". From the same table as the help and the keys.

local kit = require("../lib/kit")
local commands = require("./commands")
local filter = require("./filter")
local fuzzy = require("@pencere/fuzzy")

local b, u, S, act = kit.b, kit.u, kit.S, kit.act

local palette = {}

local P = { open = false, items = nil, state = nil, serial = 0 }
palette.state = P

local LIMIT = 40

-- The commands that can be done now, as the items of the palette: `{ key = id, title, hint = group,
-- keys }`. Made when the palette opens (while the page has the keyboard, which is what most commands
-- need) and kept until it closes.
local function items_now()
	local items = {}
	for _, command in commands.all() do
		local when = commands.when[command.when]
		if when == nil or when() then
			items[#items + 1] = {
				key = command.id,
				title = command.title,
				hint = command.group,
				keys = #command.keys > 0 and command.keys or nil,
			}
		end
	end
	return items
end

-- What the person typed as something to filter the table by: `:3000` is a port, anything else a name
-- or a word of the filter's own language.
local function as_filter(query)
	local port = string.match(query, "^:(%d+)$")
	if port ~= nil then
		return { key = "go:port:" .. port, title = string.format("Show the process that listens on port %s", port), hint = "Processes", text = "port:" .. port }
	end
	return { key = "go:" .. query, title = string.format("Filter the processes by “%s”", query), hint = "Processes", text = query }
end

-- The page's own way of finding: the commands that fuzzily match, then the query as a filter.
local function found(query, items)
	local rows = {}
	if query == "" then
		for index = 1, math.min(#items, 200) do
			rows[#rows + 1] = items[index]
		end
		return rows
	end
	for _, match in fuzzy.filter(query, items, { text = "title", limit = LIMIT }) do
		rows[#rows + 1] = match.item
	end
	rows[#rows + 1] = as_filter(query)
	return rows
end

function palette.show()
	P.items = items_now()
	-- A palette of its own each time: the host keeps the text of a field of the same name, and one
	-- opened again would begin with the words of the last.
	P.serial += 1
	P.state = { query = "", cursor = nil, field = { value = "" } }
	P.open = true
end

function palette.hide()
	P.open = false
end

function palette.view(root)
	if not P.open then
		return
	end
	root:child(u:palette("palette" .. P.serial, {
		items = P.items,
		state = P.state,
		label = "Command palette",
		hint = "Find a command, or type a name to filter the processes",
		filter = found,
		rows = 10,
		footer = {
			{ keys = "enter", title = "Do it" },
			{ keys = "escape", title = "Close" },
		},
	}))
end

-- An action of the palette: whether it was one, and its reply.
function palette.action(action)
	if type(action.from) ~= "string" or string.match(action.from, "^palette%d+$") == nil then
		return nil
	end
	if action.kind == "dismiss" then
		P.open = false
		return true
	elseif action.kind == "query" then
		-- The best match is the one the cursor goes to, as the query changes.
		P.state.cursor = nil
		return true
	elseif action.kind == "pick" then
		P.open = false
		local item = action.item
		if item.text ~= nil then
			filter.set(item.text)
			return true
		end
		return act.run(item.key)
	end
	return true
end

act.open_palette = palette.show

return palette
