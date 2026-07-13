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
-- If they don't have any contents then this warns the user too
local function setup_files()
    if not fs.exists("turtle_command/url.txt") then
        local file = fs.open("turtle_command/url.txt","w")
        file.close()
    end

    if not fs.exists("turtle_command/api_key.txt") then
        local file = fs.open("turtle_command/api_key.txt","w")
        file.close()
    end

    local url, api_key = fetch_conneciton_data()
    if url == nil then
        print("Warning: No URL in turtle_command/url.txt!")
    end

    if api_key == nil then
        print("Warning: No API key in turtle_command/api_key.txt!")
    end
end

-- Returns kind, response
-- Kind is always a string representing how to deal with response
local function parse_response(input)
    input = input.readAll()

    local kind = ""
    local response = ""
    local counter = 0

    for i in string.gmatch(input, "%S+") do
        if counter == 0 then
            kind = i
        elseif counter == 1 then
            response = i
        else
            error("Bad server response!\nKind: "..kind.."\nResponse: "..response)
        end
        counter = counter + 1
    end

    return kind, response
end

-- Returns a table containing all the items in its inventory
local function scan_own_inventory()
    local inventory = {}
    for i = 1, 16 do
        inventory[i] = turtle.getItemDetail(i)
    end

    return inventory
end

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

-- Sends a post request with all the turtle's data
local function register()
    local send_data = fetch_own_status()

    local url, api_key = fetch_conneciton_data()
    local request = {url = url.."/register", body = textutils.serialiseJSON(send_data), headers = {api_key = api_key}}
    local response, fail_reason, fail_response = http.post(request)

    if not response then
        error("\nCouldn't register with the server!\nReason: "..fail_reason)
    end

    local kind, response = parse_response(response)
    if kind == "status" and response == "successful" then
        print("Registration successful.")
    else
        error("Couldn't register with the server!\nResponse: "..kind.." "..response)
    end
end

-- Creates a websocket with the server address in url.txt
local function establish_websocket()
    local url, api_key = fetch_conneciton_data()

    -- The sub here gets rid of the "https" so that it can be replaced with "ws"
    local server_address = "ws"..url:sub(5, -1).."/websocket"
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

local function websocket_listener(websocket)
    while true do
        local response, binary = websocket.receive()

        if binary then
            error("Recieved binary response from websocket!")
        end

        print(response)
        local kind, response = parse_response(response)

        -- TODO: Deal with responses

    end
end

-- Handles the terminate event so it shuts down the websocket before terminating
local function custom_terminate(websocket)
    local event = os.pullEventRaw("terminate")
    print("Shutting down websocket.")
    websocket.close()
    if term.isColor() then
        term.setTextColor(colors.red)
    end
    print("Terminated")
    error()
end

setup_files()
register()

local websocket = establish_websocket()
coroutine.waitForAny(custom_terminate(websocket), websocket_listener(websocket))

