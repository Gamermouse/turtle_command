local download_list = {
    "turtle_command.lua",
    "install_manager.lua",
    "move_utilities.lua"
}

-- Helper function to return the server url
local function fetch_url()
    local url_file = fs.open("turtle_command/url.txt","r")
    local url = url_file.readLine()
    url_file.close()
    return url
end

for i, v in pairs(download_list) do
    local url = fetch_url()
    local response, fail_reason = http.get(url.."/lua/"..v)

    if fail_reason then
        print(fail_reason..". Getting "..v.." failed.")
    else
        local file = fs.open("turtle_command/"..v, "w")
        file.write(response.readAll())
        response.close()
        print("Got "..v..".")
    end
end

-- Removes the install manager if it is in the wrong place, e.g. on first installation
if fs.exists("install_manager.lua") and fs.exists("turtle_command/install_manager.lua") then
    fs.delete("install_manager.lua")
end

shell.run("turtle_command/turtle_command.lua")