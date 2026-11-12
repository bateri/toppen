-- What every file of the page shares: the kit's builders, the library's context `u` (which
-- remembers the pointer and the keyboard and gives the components), the page's state, the reply that
-- what the page does is gathered into, and the few pieces the page is made of more than once (an
-- icon, the keys of a command).

local keys = require("@pencere/keys")
local lib = require("@pencere/ui")

local b = pencere
-- Compact: a monitor is a dense page, and the rows of its table are 22 points.
local u = lib.new({ density = "compact" })

-- The page's state: what the program said and what the person did, in one place, so that `render` is
-- a function of it.
local S = {
	now = 0,
	width = 1000,
	height = 700,
	-- The width inside the panel that holds the table, the ports or a card's details.
	inner = 976,
	-- "wide", "medium" or "narrow", by the page's width; `short` for a page with little height.
	layout = "wide",
	short = false,
	-- What `boot` said, and why there is none.
	boot = nil,
	boot_error = nil,
	-- The settings say the machine's name and the person's own are not shown.
	private = false,
	-- Seconds between two samples, and whether updates are paused (`sampling`).
	interval = 1,
	paused = false,
	-- What the machine did lately, each series newest last (`overview`), and the sample last told.
	series = { at = {}, cpu = {}, mem = {}, swap = {}, rx = {}, tx = {}, read = {}, write = {}, gpu = {} },
	-- Each logical processor's share, one series each; and the network's by interface name.
	cores = {},
	-- Each GPU's load, one series each.
	gpus = {},
	ifaces = {},
	iface_count = 0,
	sample = nil,
	-- The card of the overview whose details are open, if one is.
	open_card = nil,
	-- Calls out that the person waits on: a quiet spinner.
	busy = 0,
	-- The processes the person marked, by pid; the table shows a dot before their names.
	marks = {},
}

local SIZE = {
	status = u.theme.size.status,
	-- The room around the cards and the panel under them, and between them, points.
	pad = 12,
	gap = 10,
	-- The corners of the cards and of the panel.
	radius = 10,
	-- The widths the page changes its layout at, points.
	narrow = 640,
	medium = 1000,
	-- A page this short shows less around the cards.
	short = 360,
	-- How many samples the page keeps of each series.
	kept = 900,
}

-- --- What the page does is gathered into one reply ---------------------------------------------
--
-- What an action asks of the host (the keyboard, the pasteboard, a link) is put here as it is
-- decided, wherever in the page that is, and every way into the page (an event, an answer of the
-- program, a timer) hands it out with its own reply.

local gathered = nil

local function gather(reply)
	gathered = b.merge(gathered, reply)
end

-- The reply of a way into the page: `reply` with what was gathered.
local function take(reply)
	local out = gathered
	gathered = nil
	return b.merge(out, reply)
end

-- Asks the program to do `method`; `done(result, err)` is called with its answer, and what it
-- gathered goes out with that answer's reply. `err` is { code =, message = }.
local function call(method, params, done)
	return b.call(method, params, function(result, err)
		local reply = nil
		if done then
			reply = done(result, err)
		end
		return take(reply == nil and true or reply)
	end)
end

-- --- Words and pieces ------------------------------------------------------------------------------

-- Whether the page is drawn in the character cells of a terminal: what is a few points off a whole row
-- is a row more or none there.
local cells = nil
local function in_cells()
	if cells == nil then
		cells = pencere.draws().cells == true
	end
	return cells
end

-- Whether the page is on the cells of a terminal that has few rows (fewer than 33): there the rows that
-- only keep things apart (above the panel, round the bar of the table) are rows the table does without.
-- A taller terminal has the rows to spare.
local function tight()
	return in_cells() and S.height < 600
end

-- Whether a picture of the page is drawn in too few pixels to read a small one by (a terminal with no
-- graphics protocol, which draws it in half blocks): there an icon is a character, which is drawn
-- exactly. Asked the first time it is needed, not when the file is read.
local coarse = nil
local function coarse_pictures()
	if coarse == nil then
		coarse = pencere.draws().coarse_images == true
	end
	return coarse
end

-- The characters that say what the icons of src/assets/icons do, where a picture is coarse. Each is in
-- the fonts a terminal has by default (Menlo and SF Mono) but the magnifier, which Menlo has and SF Mono
-- falls back from.
local CHARACTERS = {
	search = "⌕",
	["chevron-right"] = "▸",
	["chevron-down"] = "▾",
	["loader-2"] = "…",
	eye = "○",
	["eye-off"] = "●",
}

-- One of the pictures of src/assets/icons (Tabler's, MIT), `size` points, in a colour of the theme; or
-- where a picture is coarse, the character that says it, in that colour.
local function icon(name, size, color)
	if coarse_pictures() then
		return b.text(CHARACTERS[name] or "•"):color(color or "dim"):no_shrink()
	end
	size = size or 15
	return b.image("assets/icons/" .. name .. ".svg"):size(size, size):tint(color or "dim"):no_shrink()
end

-- The icon a button of the library takes for `name`: the path of its picture, which the button tints
-- in the colour it draws in; or where a picture is coarse, the element of the character that says it.
local function icon_for(name)
	if coarse_pictures() then
		return icon(name)
	end
	return "assets/icons/" .. name .. ".svg"
end

-- The keys of a command as the host's keyboard writes them (`@pencere/keys`: "cmd+shift+k" is "⇧⌘K"
-- on a Mac, "Super+Shift+K" elsewhere). `spec` is the words of a command's keys, or a list of them:
-- the first is the one shown.
local function keys_of(spec)
	if type(spec) == "table" then
		spec = spec[1]
	end
	if not spec then
		return nil
	end
	return keys.label(spec)
end

-- The name of a user as the page shows it: the person's own is "you" where the settings keep it
-- private.
local function user_shown(user)
	if S.private and S.boot ~= nil and user == S.boot.person.name then
		return "you"
	end
	return user
end

-- The name of the machine as the page shows it.
local function host_shown(host)
	return S.private and "this machine" or host
end

-- A picture turning round, for what is being waited for: the host turns it, a frame at a time. Where the
-- page is drawn in cells there is no turning of a picture (it would stand still and look stuck): the busy
-- mark is a character that is another every tenth of a second, which a timer asks the page to draw for
-- as long as it is drawn.
local TURNING = { "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏" }
local turn = { step = 0, seen = 0, timer = nil }
local function spinner(size, color)
	if in_cells() then
		turn.seen = S.now
		if turn.timer == nil or not turn.timer:active() then
			turn.timer = pencere.time.every(0.1, function()
				turn.step += 1
				if S.now - turn.seen > 0.5 then
					turn.timer:cancel()
					return nil
				end
				return true
			end)
		end
		return b.text(TURNING[turn.step % #TURNING + 1]):color(color or "dim"):no_shrink()
	end
	local turning = icon("loader-2", size, color)
	if coarse_pictures() then
		return turning
	end
	return turning:motion({ spin = 1 })
end

-- The actions of the page, by name, filled by the parts that do them: a part calls another's action
-- through here, so that no two parts require each other.
local act = {}

return {
	b = b,
	u = u,
	lib = lib,
	S = S,
	SIZE = SIZE,
	act = act,
	gather = gather,
	take = take,
	call = call,
	icon = icon,
	icon_for = icon_for,
	in_cells = in_cells,
	tight = tight,
	keys_of = keys_of,
	user_shown = user_shown,
	host_shown = host_shown,
	spinner = spinner,
}
