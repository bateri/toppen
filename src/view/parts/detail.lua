-- The panel of one process, beside the table (or over it, where the pane is narrow): its name and
-- state, what it has taken lately as a graph, the whole command line, where it works, who started it
-- and what it started, and its environment. What the program tells of it is `detail` once and the
-- topic `process` with each sample; the history the graphs draw is the page's, from that topic.
--
-- The panel follows the cursor of the table. A click on a parent or a child shows that process
-- instead, and the table's cursor goes to it where it is in the list.

local kit = require("../lib/kit")
local format = require("../lib/format")
local graph = require("./graph")
local commands = require("./commands")
local tab = require("./table")

local b, u, S, act = kit.b, kit.u, kit.S, kit.act

local detail = {}

local PAD = 12
local EDGE = 7
-- A pane narrower than this has the panel over the table, not beside it.
local NARROW_BELOW = 760
-- How many samples the graphs keep (and show).
local KEPT = 90
-- How often what the panel reads once (its children, its open files) is read again, seconds.
local AGAIN = 3
-- How many of the parents, the children and the variables are listed.
local LISTED = { parents = 6, children = 8, env = 80 }

local D = {
	open = false,
	-- The process shown: { pid =, start = }, and the name it had when it was asked for.
	key = nil,
	name = "",
	-- What `detail` said, and why there is none.
	info = nil,
	problem = nil,
	-- The process is no more.
	ended = false,
	-- The last of `process`, and the history of it.
	live = nil,
	series = { cpu = {}, mem = {} },
	-- The values of the variables that look like secrets are shown.
	reveal = false,
	-- The variables, by name, whose secret value the person showed one at a time.
	revealed = {},
	-- The subscription, and the number of the look that is to come again.
	sub = nil,
	round = 0,
	-- The split's own state: the width the person dragged, and the drawer of a narrow pane.
	split = { size = 400, drawer = false },
}
detail.state = D

local STATES = {
	R = "running",
	S = "sleeping",
	D = "waiting on a disk",
	T = "stopped",
	Z = "a zombie",
	I = "idle",
}

local function same_key(pid, start)
	return D.key ~= nil and D.key.pid == pid and D.key.start == start
end

local function push(series, value)
	series[#series + 1] = value
	if #series > KEPT then
		table.remove(series, 1)
	end
end

-- --- Asking the program --------------------------------------------------------------------------------

local function fetch(key)
	kit.call("detail", { pid = key.pid, start = key.start }, function(result, err)
		if D.key ~= key then
			return true
		end
		if result then
			D.info = result
			D.problem = nil
			D.name = result.name
		elseif err and err.code == "gone" then
			D.ended = true
			D.problem = err.message
		elseif err then
			D.problem = err.message
		end
		return true
	end)
end

-- Looks again at what the panel reads once, while it is open and on the same process.
local function again(key, round)
	b.time.after(AGAIN, function()
		if D.open and D.key == key and D.round == round and not D.ended then
			fetch(key)
			again(key, round)
		end
		return kit.take(true)
	end)
end

local function watch(key)
	D.sub = b.subscribe("process", { pid = key.pid, start = key.start }, function(told)
		if D.key ~= key then
			return nil
		end
		if told.gone then
			D.ended = true
		else
			D.live = told
			push(D.series.cpu, told.cpu)
			push(D.series.mem, told.mem)
		end
		return kit.take(true)
	end)
end

local function unwatch()
	if D.sub ~= nil then
		b.cancel(D.sub)
		D.sub = nil
	end
end

-- Shows the process `pid` started at `start`. `drawer` opens the narrow pane's drawer too.
function detail.show(pid, start, name, drawer)
	if D.open and same_key(pid, start) then
		D.split.drawer = D.split.drawer or drawer == true
		return
	end
	unwatch()
	D.open = true
	D.key = { pid = pid, start = start }
	D.name = name or ""
	D.info, D.problem, D.live, D.ended, D.reveal = nil, nil, nil, false, false
	D.revealed = {}
	D.series = { cpu = {}, mem = {} }
	D.round += 1
	if drawer then
		D.split.drawer = true
	end
	fetch(D.key)
	watch(D.key)
	again(D.key, D.round)
end

function detail.close()
	if not D.open then
		return
	end
	unwatch()
	D.open, D.key, D.info, D.live = false, nil, nil, nil
	D.split.drawer = false
	D.round += 1
	kit.gather(u:focus(u:name("procs", "body")))
end

-- The row of the table opened (Enter, a double click): its process shown.
function detail.open_row(row)
	if row ~= nil and not row.group then
		detail.show(row.pid, row.start, row.name, true)
	end
end

-- The cursor of the table moved: the panel follows it, if it is open.
function detail.follow(row)
	if D.open and row ~= nil and not row.group then
		detail.show(row.pid, row.start, row.name, false)
	end
end

-- Opens the panel on the process under the cursor, or closes it.
function detail.toggle()
	if D.open then
		detail.close()
		return
	end
	local row = tab.current()
	if row ~= nil then
		detail.open_row(row)
	end
end

-- The process a command is about: the one the panel shows, else the one under the cursor.
-- { pid, start, name, user, command, children (nil where not known), nice, mine, cwd }.
function detail.target()
	-- The ports shown say whose process it is: the row under their cursor.
	if act.ports_target ~= nil then
		local target = act.ports_target()
		if target ~= nil then
			return target
		end
	end
	local mine = function(user)
		local person = S.boot.person
		return person.root or person.name == user
	end
	if D.open and D.key ~= nil then
		local info, live = D.info, D.live
		return {
			pid = D.key.pid,
			start = D.key.start,
			name = D.name,
			user = info and info.user or "",
			command = info and table.concat(info.argv, " ") or D.name,
			children = info and info.childrenTotal or nil,
			nice = live and live.nice or (info and info.nice or 0),
			mine = info ~= nil and info.may or false,
			cwd = info and info.cwd or nil,
			ended = D.ended,
		}
	end
	local row = tab.current()
	if row == nil then
		return nil
	end
	return {
		pid = row.pid,
		start = row.start,
		name = row.name,
		user = row.user,
		command = row.command,
		children = tab.view_mode() ~= "flat" and row.children or nil,
		nice = row.nice,
		mine = mine(row.user),
		cwd = nil,
		ended = row.exited == true,
	}
end

-- The details of the target (`cwd` among them): at once where the panel has them, else asked for.
function detail.with_info(target, done)
	if D.open and D.key ~= nil and D.info ~= nil and same_key(target.pid, target.start) then
		done(D.info)
		return
	end
	kit.call("detail", { pid = target.pid, start = target.start }, function(result)
		done(result)
		return true
	end)
end

-- --- Drawing -----------------------------------------------------------------------------------------------

local function label(words)
	return b.text(words):text_size(11):color("quiet"):weight(600)
end

local function field(title, body)
	return b.div():col():gap(3):min_w(0):child(label(title)):child(body)
end

local function quiet(words)
	return b.text(words):color("quiet"):text_size(12)
end

-- A line of a list that goes to another process when pressed.
local function link_row(link, prefix)
	local name = string.format("detail-go/%d/%d/%s", link.pid, link.start, prefix)
	local over = u:hovered(name)
	local row = b.div():row():items_center():gap(8):h(22):px(6):rounded(4):min_w(0)
		:press(name):role("button"):label(link.name .. ", process " .. link.pid)
		:bg(over and b.alpha("foreground", 0.08) or nil)
	row:child(b.text(link.name):truncate():min_w(0):grow(1))
	row:child(b.text(string.format("%.1f%%", link.cpu)):color("quiet"):text_size(12):no_shrink())
	row:child(b.text(tostring(link.pid)):color("quiet"):text_size(12):no_shrink())
	return row
end

local function list_of(links, shown, total, prefix)
	local box = b.div():col():min_w(0)
	for index = 1, math.min(#links, shown) do
		box:child(link_row(links[index], prefix))
	end
	if total > math.min(#links, shown) then
		box:child(b.div():px(6):child(quiet(string.format("and %d more", total - math.min(#links, shown)))))
	end
	return box
end

-- The graph of a series with its latest value over it.
local function history(title, value, series, role, ceiling, inner)
	local head = b.div():row():items_center():gap(8)
	head:child(label(title))
	head:child(b.spacer())
	head:child(b.text(value):weight(600):color(role):no_shrink())
	local box = b.div():col():gap(3):child(head)
	box:child(b.div():rounded(4):bg("raised1"):clip():child(graph.area({ { values = series, role = role } }, KEPT, ceiling, inner, 44)))
	return box
end

local function environment(info, inner)
	local box = b.div():col():gap(4):min_w(0)
	local secrets = 0
	for _, entry in info.env do
		if entry.secret then
			secrets += 1
		end
	end
	local head = b.div():row():items_center():gap(8)
	head:child(label(string.format("Environment · %d", #info.env)))
	head:child(b.spacer())
	if secrets > 0 then
		head:child(u:button("env-reveal", {
			title = D.reveal and "Hide secrets" or string.format("Show %d secrets", secrets),
			icon = D.reveal and "assets/icons/eye-off.svg" or "assets/icons/eye.svg",
			tip = "Values whose names look like passwords, tokens or keys are hidden until shown",
		}))
	end
	box:child(head)
	if #info.env == 0 then
		box:child(quiet("The system does not tell this process's environment."))
		return box
	end
	for index = 1, math.min(#info.env, LISTED.env) do
		local entry = info.env[index]
		local shown = not entry.secret or D.reveal or D.revealed[entry.name]
		local line = b.div():row():items_center():gap(0):min_w(0):min_h(22):family("terminal"):text_size(12)
		line:child(b.text(entry.name):color("dim"):no_shrink())
		line:child(b.text("="):color("quiet"):no_shrink())
		if shown then
			line:child(b.text(entry.value):truncate():min_w(0):grow(1))
		else
			line:child(b.text("••••••••"):color("quiet"):no_shrink())
			line:child(b.spacer())
		end
		if entry.secret then
			line:child(u:icon_button("env-eye-" .. index, {
				icon = shown and "assets/icons/eye-off.svg" or "assets/icons/eye.svg",
				label = (shown and "Hide " or "Show ") .. entry.name,
				size = 20,
			}))
		end
		box:child(line)
	end
	if #info.env > LISTED.env then
		box:child(quiet(string.format("and %d more", #info.env - LISTED.env)))
	end
	return box
end

local function buttons(target)
	local row = b.div():row():wrap():gap(6)
	local why = nil
	if target ~= nil and not target.mine then
		why = string.format("Needs root: this process belongs to %s", target.user ~= "" and kit.user_shown(target.user) or "someone else")
	end
	local gone = D.ended
	row:child(u:button("do-signal", { title = "Signal", keys = commands.first("signal"), disabled = why ~= nil or gone, tip = why }))
	row:child(u:button("do-renice", { title = "Priority", keys = commands.first("renice"), disabled = why ~= nil or gone, tip = why }))
	row:child(u:button("do-shell", { title = "Shell here", keys = commands.first("shell"), disabled = gone }))
	row:child(u:button("do-copy_command", { title = "Copy command", keys = commands.first("copy_command") }))
	return row
end

local function scrolled(box)
	return b.div():id("detail/body"):scroll():grow(1):min_h(0):child(box)
end

local function panel()
	local width = math.min(D.split.size, S.inner - 40)
	local inner = width - 2 * PAD - 8
	local info, live = D.info, D.live

	-- The head stays where it is when the rest scrolls: the name, the way out and the buttons.
	local top = b.div():col():gap(8):px(PAD):pt(PAD):pb(8):min_w(0):no_shrink()
	local head = b.div():row():items_center():gap(8):min_w(0)
	head:child(b.text(D.name):text_size(15):weight(650):truncate():min_w(0))
	if D.ended then
		head:child(u:badge("Ended", { tone = "warn" }))
	elseif live and live.state == "T" then
		head:child(u:badge("Stopped", { tone = "warn" }))
	elseif live and live.state == "Z" then
		head:child(u:badge("Zombie", { tone = "bad" }))
	end
	head:child(b.spacer())
	head:child(u:icon_button("detail-close", { icon = "close", label = "Close the details", keys = commands.first("detail") }))
	top:child(head)
	local root = b.div():size_full():col():min_h(0):child(top)

	if info == nil then
		local body = b.div():col():gap(8):px(PAD):min_w(0)
		if D.ended then
			body:child(b.text(D.problem or "This process has ended."):color("dim"):wrap())
		else
			body:child(b.div():row():items_center():gap(8):child(kit.spinner(14, "quiet")):child(quiet("Reading the process")))
		end
		return root:child(body)
	end

	-- Who it is.
	local state = live and live.state or info.state
	local meta = string.format("pid %d · %s · %s", info.pid, kit.user_shown(info.user), STATES[state] or state)
	if live and live.at and info.start > 0 and live.at > info.start then
		meta = meta .. " · started " .. format.duration(live.at - info.start) .. " ago"
	end
	top:child(b.text(meta):color("dim"):wrap())
	top:child(buttons(detail.target()))
	root:child(u:rule("x"))

	local box = b.div():col():gap(12):p(PAD):min_w(0)

	-- What it takes.
	local cpu = live and live.cpu or info.cpu
	local mem = live and live.mem or info.mem
	local machine = S.boot.machine
	box:child(history(
		"CPU",
		string.format("%.1f%%", cpu),
		D.series.cpu,
		format.load_role(cpu / math.max(machine.threads, 1)),
		graph.ceiling({ { values = D.series.cpu } }, KEPT, 100),
		inner
	))
	box:child(history(
		"Memory",
		format.short_bytes(mem),
		D.series.mem,
		"accent",
		graph.ceiling({ { values = D.series.mem } }, KEPT, 8 * 1024 * 1024) * 1.1,
		inner
	))
	local numbers = b.div():row():wrap():gap_x(14):gap_y(2):text_size(12):color("dim")
	numbers:child(b.text(string.format("%d threads", live and live.threads or info.threads)))
	numbers:child(b.text("virtual " .. format.short_bytes(info.virt)))
	numbers:child(b.text("time " .. format.cpu_time(live and live.time or info.time)))
	numbers:child(b.text(string.format("nice %d", live and live.nice or info.nice)))
	if info.fds ~= nil then
		numbers:child(b.text(string.format("%d open files", info.fds)))
	end
	local read, write = live and live.read or info.read, live and live.write or info.write
	if read >= 1 or write >= 1 then
		numbers:child(b.text(string.format("disk %s read · %s written", format.rate(read), format.rate(write))))
	end
	box:child(numbers)

	-- Where it comes from.
	box:child(field("Command", b.text(table.concat(info.argv, " ")):wrap():family("terminal"):text_size(12):min_w(0)))
	if info.exe ~= nil and info.exe ~= info.argv[1] then
		box:child(field("Program", b.text(info.exe):wrap():family("terminal"):text_size(12):color("dim"):min_w(0)))
	end
	box:child(field(
		"Working folder",
		info.cwd and b.text(info.cwd):wrap():family("terminal"):text_size(12):color("dim"):min_w(0)
			or quiet("The system does not tell it.")
	))
	if info.ports ~= nil and #info.ports > 0 then
		local lines = b.div():col():gap(2)
		for _, one in info.ports do
			local host = string.find(one.addr, ":", 1, true) and ("[" .. one.addr .. "]") or one.addr
			lines:child(b.text(string.format("%s  %s:%d", one.proto, host, one.port)):family("terminal"):text_size(12):color("dim"):truncate():min_w(0))
		end
		box:child(field("Listening on", lines))
	end
	if #info.parents > 0 then
		box:child(field("Started by", list_of(info.parents, LISTED.parents, #info.parents, "up")))
	end
	if info.childrenTotal > 0 then
		box:child(field(string.format("Started · %d", info.childrenTotal), list_of(info.children, LISTED.children, info.childrenTotal, "down")))
	end
	box:child(environment(info, inner))
	return root:child(scrolled(box))
end

-- The width the table has, as the panel takes what it takes.
function detail.table_width(width)
	if D.open and width >= NARROW_BELOW then
		return width - D.split.size - EDGE
	end
	return width
end

-- The table with the panel beside it, if it is open.
function detail.view(table_element)
	if not D.open then
		return table_element
	end
	return u:split("detail", {
		state = D.split,
		side = "end",
		min = 300,
		max = 620,
		default = 400,
		label = "Width of the details",
		narrow_below = NARROW_BELOW,
		first = table_element,
		second = panel(),
	})
end

-- An action of the panel: the split's drawer, the close button, the reveal button.
function detail.action(action)
	local from, kind = action.from, action.kind
	if from == "detail" then
		if kind == "drawer" and action.open == false then
			detail.close()
		end
		return true
	elseif from == "detail-close" and kind == "press" then
		detail.close()
		return true
	elseif from == "env-reveal" and kind == "press" then
		D.reveal = not D.reveal
		D.revealed = {}
		return true
	elseif string.sub(from, 1, 8) == "env-eye-" and kind == "press" then
		local entry = D.info and D.info.env[tonumber(string.sub(from, 9))]
		if entry ~= nil then
			if D.reveal then
				-- Every value was shown: hiding one hides it alone.
				D.reveal = false
				for _, other in D.info.env do
					if other.secret then
						D.revealed[other.name] = true
					end
				end
			end
			D.revealed[entry.name] = (not D.revealed[entry.name]) or nil
		end
		return true
	end
	return nil
end

-- A press no component took: a parent or a child of the process.
function detail.event(event)
	if event.type ~= "press" or (event.button or "left") ~= "left" then
		return nil
	end
	local over = b.layout.hit({ x = event.x, y = event.y })
	if over == nil then
		return nil
	end
	local pid, start = string.match(over, "^detail%-go/(%d+)/(%d+)/")
	if pid == nil then
		return nil
	end
	pid, start = tonumber(pid), tonumber(start)
	local name = ""
	for _, links in { D.info and D.info.parents or {}, D.info and D.info.children or {} } do
		for _, link in links do
			if link.pid == pid and link.start == start then
				name = link.name
			end
		end
	end
	detail.show(pid, start, name, false)
	tab.select_key(pid, start)
	return true
end

act.detail_open = detail.open_row
act.follow = detail.follow

return detail
