local mv = require("move_utilities")

-- Helper function to return url, api_key
local function fetch_conneciton_data()
    local url_file = fs.open("turtle_command/url.txt","r")
    local url = url_file.readLine()
    url_file.close()
    local api_key_file = fs.open("turtle_command/api_key.txt","r")
    local api_key = api_key_file.readLine()
    api_key_file.close()
    return url, api_key
end

-- Makes sure that all the files that must exist, do
-- If they don't have any contents then this warns the user too\
-- In the case of the direction file, it will not allow the user to continue the program unless it has a direciton in it (n, s, e, w)
local function setup_files()
    if not fs.exists("turtle_command/url.txt") then
        local file = fs.open("turtle_command/url.txt","w")
        file.close()
    end

    if not fs.exists("turtle_command/api_key.txt") then
        local file = fs.open("turtle_command/api_key.txt","w")
        file.close()
    end

    if not fs.exists("turtle_command/facing.txt") then
        local file = fs.open("turtle_command/facing.txt","w")
        file.close()
    end

    local url, api_key = fetch_conneciton_data()
    if url == nil then
        print("Warning: No URL in turtle_command/url.txt!")
    end

    if api_key == nil then
        print("Warning: No API key in turtle_command/api_key.txt!")
    end

    if mv.read_first_line("turtle_command/facing.txt") == nil then
        if term.isColor() then
            term.setTextColor(colors.red)
        end
        print("Error: No direction key in turtle_command/facing.txt, you must manually insert the direction this turtle is facing!")
        error()
    end
end

-- Returns instruction, data
-- Kind is always a string representing how to deal with response
local function parse_response(input)
    local decoded_json = textutils.unserialiseJSON(input)
    print(decoded_json.instruction, decoded_json.data)
    return decoded_json.instruction, decoded_json.data
end

-- Returns a table containing all the items in its inventory
local function scan_own_inventory()
    local inventory = {}
    for i = 1, 16 do
        inventory[i] = turtle.getItemDetail(i)
    end

    return inventory
end

-- Gets a bunch of data about this turtle
local function fetch_own_status()
    local x, y, z = gps.locate(1)

    if not x then
        error("Coudln't get gps!")
    end

    local computer_id = os.getComputerID()
    local equipped_left = turtle.getEquippedLeft()
    local equipped_right = turtle.getEquippedRight()
    local inventory = scan_own_inventory()

    local fuel = turtle.getFuelLevel()

    -- we set connected to true here as if this message gets sent, then we must be connected
    if textutils.serialiseJSON(inventory) == "{}" then
        inventory = nil
    end
    local my_data = {id = computer_id, connected = true, equipped_left = equipped_left, equipped_right = equipped_right, coordinates = {x = x, y = y, z = z}, inventory_contents = inventory, inventory_size = 16, fuel = fuel}

    return my_data
end

-- Utility function to change a direction as a letter to a coordinate number. E.g. turns "n" into a Z offset of -1
-- Returns offset_x, offset_z
local function facing_offset()
    local facing = mv.read_first_line("turtle_command/facing.txt")
    local offset_x = 0
    local offset_z = 0
    if facing == "n" then
        offset_z = -1
    elseif facing == "s" then
        offset_z = 1
    else
        offset_z = 0
    end
    if facing == "e" then
        offset_x = 1
    elseif facing == "w" then
        offset_x = -1
    else
        offset_x = 0
    end
    return offset_x, offset_z
end

-- Adds a block to the block cache
local function append_block_cache(block_cache)
    -- If the file doesnt exist, create it
    if not fs.exists("turtle_command/block_cache.txt") then
        local f = fs.open("turtle_command/block_cache.txt", "w")
        f.close()
    end

    local f = fs.open("turtle_command/block_cache.txt", "r")
    local block_cash_old = textutils.unserialise(f.readAll())
    f.close()

    -- If there werent any blocks in the old cache, block_cash_old = nil, causing an error, thus this is here.
    if not block_cash_old then
        block_cash_old = {}
    end

    -- This is the structure of an item in block cache: {name = data.name, x = x, y = y, z = z}
    -- data.name is the name of a block
    for i, v in pairs(block_cache) do
        block_cash_old[#block_cash_old+1] = v
    end

    f = fs.open("turtle_command/block_cache.txt", "w")
    f.write(textutils.serialise(block_cash_old))
    f.close()
end

-- Uses turtle.inspect to inspect all 6 blocks around it, appending them to the block cache.
local function append_inspect_all()
    local x, y, z = gps.locate()

    local offset_x, offset_z = facing_offset()

    local block_cache = {}
    local has_block, data = turtle.inspectUp()
    if has_block then block_cache[#block_cache+1] = {{name = data.name, states = data.state}, {x = x, y = y+1, z = z}}
    else block_cache[#block_cache+1] = {{name = "minecraft:air", states = {}}, {x = x, y = y+1, z = z}} end

    local has_block, data = turtle.inspectDown()
    if has_block then block_cache[#block_cache+1] = {{name = data.name, states = data.state}, {x = x, y = y-1, z = z}}
    else block_cache[#block_cache+1] = {{name = "minecraft:air", states = {}}, {x = x, y = y-1, z = z}} end

    local has_block, data = turtle.inspect()
    if has_block then block_cache[#block_cache+1] = {{name = data.name, states = data.state}, {x = x + offset_x, y = y, z = z + offset_z}}
    else block_cache[#block_cache+1] = {{name = "minecraft:air", states = {}}, {x = x + offset_x, y = y, z = z + offset_z}} end

    mv.left()

    local offset_x, offset_z = facing_offset()

    local has_block, data = turtle.inspect()
    if has_block then block_cache[#block_cache+1] = {{name = data.name, states = data.state}, {x = x + offset_x, y = y, z = z + offset_z}}
    else block_cache[#block_cache+1] = {{name = "minecraft:air", states = {}}, {x = x + offset_x, y = y, z = z + offset_z}} end

    mv.left()

    local offset_x, offset_z = facing_offset()

    local has_block, data = turtle.inspect()
    if has_block then block_cache[#block_cache+1] = {{name = data.name, states = data.state}, {x = x + offset_x, y = y, z = z + offset_z}}
    else block_cache[#block_cache+1] = {{name = "minecraft:air", states = {}}, {x = x + offset_x, y = y, z = z + offset_z}} end

    mv.left()

    local offset_x, offset_z = facing_offset()

    local has_block, data = turtle.inspect()
    if has_block then block_cache[#block_cache+1] = {{name = data.name, states = data.state}, {x = x + offset_x, y = y, z = z + offset_z}}
    else block_cache[#block_cache+1] = {{name = "minecraft:air", states = {}}, {x = x + offset_x, y = y, z = z + offset_z}} end

    mv.left()

    append_block_cache(block_cache)
end

-- Utility function, used such that each time the turtle moves it will cache the blocks directly above and below it.
-- Better than running append_inspect_all() each time as that slows the turtle down massively.
local function cache_updown_move()
    local x, y, z = gps.locate()

    local block_cache = {}
    local has_block, data = turtle.inspectUp()
    if has_block then block_cache[#block_cache+1] = {{name = data.name, states = data.state}, {x = x, y = y+1, z = z}}
    else block_cache[#block_cache+1] = {{name = "minecraft:air", states = {}}, {x = x, y = y+1, z = z}} end

    local has_block, data = turtle.inspectDown()
    if has_block then block_cache[#block_cache+1] = {{name = data.name, states = data.state}, {x = x, y = y-1, z = z}}
    else block_cache[#block_cache+1] = {{name = "minecraft:air", states = {}}, {x = x, y = y-1, z = z}} end

    append_block_cache(block_cache)

    return true
end

local function format_message(instruction, data)
    local message = {instruction = instruction, data = data}
    return textutils.serialiseJSON(message)
end

-- Opens the block cache file, sends all of the data to the server, then clears the file.
local function send_block_cache(websocket)
    local f = fs.open("turtle_command/block_cache.txt", "r")
    local block_cache = f.readAll()
    f.close()
    if not block_cache then
        return
    end

    block_cache = textutils.unserialise(block_cache)

    f = fs.open("turtle_command/block_cache.txt","w")
    f.close()

    if block_cache then
        local message = format_message("sendBlocks", textutils.serialiseJSON(block_cache))
        websocket.send(message)
    end
end

-- Sends a post request with all the turtle's data
local function ws_register(websocket)
    if not websocket then
        print("A")
    end
    local send_data = fetch_own_status()
    local message = format_message("register", textutils.serialiseJSON(send_data))
    websocket.send(message)
end

-- Creates a websocket with the server address in url.txt
local function establish_websocket()
    local url, api_key = fetch_conneciton_data()

    -- The sub here gets rid of the "https" so that it can be replaced with "ws"
    -- Note: We also submit the ID so the rust server can track which websocket is which
    local server_address = "ws"..url:sub(5, -1).."/websocket?id="..os.getComputerID()
    print("Establishing websocket connection to "..server_address)
    local socket, fail_reason = http.websocket({url = server_address, timeout = 5, headers = {api_key = api_key}})

    if not socket then
        print(fail_reason)
        error("Couldn't make connection.")
    else
        print("Websocket connected!")
    end

    return socket
end

-- Handles movement instructions
local function handle_move(data)
    if data == "turnLeft" or data == "left" or data == "l" then
        mv.left()
    elseif data == "turnRight" or data == "right" or data == "r" then
        mv.right()
    elseif data == "forward" or data == "f" then
        turtle.forward()
    elseif data == "up" or data == "u" then
        turtle.up()
    elseif data == "down" or data == "d" then
        turtle.down()
    end
end

-- Handles run length encoding paths
-- Data should be formatted as such:
-- (letter)(number) etc...
-- for example, l4r5u12d1rl means left 4, right 5, up 12, down 1, right, left
local function handle_path(data)
    local raw_list={}
    data:gsub("%a%d*",function(c) table.insert(raw_list, c) end)

    for i, v in pairs(raw_list) do
        local action = string.sub(v, 1, 1)
        local count = tonumber(string.sub(v, 2, -1))

        -- Handles the single letter commands
        if count == nil then
            count = 1
        end

        print(v)

        for c = 1, count do
            handle_move(action)
        end
    end
end

local function handle_websocket_message(event_data, websocket)
    local url = event_data[1]
    local message = event_data[2]
    local is_binary = event_data[3]

    if is_binary then
        error("Recieved binary response from websocket!")
    end

    local kind, data = parse_response(message)

    if kind == "move" then
        handle_move(data)
    elseif kind == "movementPath" then
        handle_path(data)
    elseif kind == "register" then
        ws_register(websocket)
    elseif kind == "testBlockSend" then -- DEBUG
        append_inspect_all()
        send_block_cache(websocket)
    end

    -- TODO: Deal with more responses
end

-- Handles the terminate event so it shuts down the websocket before terminating
local function handle_terminate(websocket)
    websocket.close()
    print("Websocket shut down.")
    if term.isColor() then
        term.setTextColor(colors.red)
    end
    print("Terminated")
    error()
end

local function persistent_event_handler(websocket)
    -- Contains coroutines so we can e.g. handle events and move at the same time
    local tasks = {}
    while true do
        local event_data = table.pack(os.pullEventRaw())
        local event = table.remove(event_data, 1)

        if event == "terminate" then
            handle_terminate(websocket)
        elseif event == "websocket_message" then
            handle_websocket_message(event_data, websocket)
        end
    end
end

setup_files()

local websocket = establish_websocket()
ws_register(websocket)
persistent_event_handler(websocket)

