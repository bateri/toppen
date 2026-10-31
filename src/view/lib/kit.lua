-- What every file of the page shares: the kit's builders, the one `Ui` that remembers the pointer,
-- the page's state, the reply that what the page does is gathered into, and the few pieces the page
-- is made of more than once (an icon, a button, the keys of a command).

local keys = require("@pencere/keys")

local b = pencere
local ui = b.Ui.new()

-- The page's state: what the program said and what the person did, in one place, so that `render` is
-- a function of it.
local S = {
	now = 0,
	width = 1000,
	height = 700,
	-- "wide", "medium" or "narrow", by the page's width; `short` for a page with little height.
	layout = "wide",
	short = false,
	-- What `boot` said, and why there is none.
	boot = nil,
	boot_error = nil,
	-- Seconds between two samples, and whether updates are paused (`sampling`).
	interval = 1,
	paused = false,
	-- What the machine did lately, each series newest last (`overview`), and the sample last told.
	series = { cpu = {}, mem = {}, swap = {}, rx = {}, tx = {}, read = {}, write = {} },
	sample = nil,
	-- Calls out that the person waits on: a quiet spinner.
	busy = 0,
}

local SIZE = {
	status = 24,
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

-- Whether the pointer is over the element named `id`.
local function hovered(id)
	return ui.hover == id
end

-- One of the pictures of src/assets/icons (Tabler's, MIT), `size` points, in a colour of the theme.
local function icon(name, size, color)
	size = size or 15
	return b.image("assets/icons/" .. name .. ".svg"):size(size, size):tint(color or "dim"):no_shrink()
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

-- A picture turning round, for what is being waited for: the host turns it, a frame at a time.
local function spinner(size, color)
	return icon("loader-2", size, color):motion({ spin = 1 })
end

-- The actions of the page, by name, filled by the parts that do them: a part calls another's action
-- through here, so that no two parts require each other.
local act = {}

return {
	b = b,
	ui = ui,
	S = S,
	SIZE = SIZE,
	act = act,
	gather = gather,
	take = take,
	call = call,
	hovered = hovered,
	icon = icon,
	keys_of = keys_of,
	spinner = spinner,
}
