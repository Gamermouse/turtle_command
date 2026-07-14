-- Opens, reads, and returns the first line of a file.
local function read_first_line(filepath)
    local f = fs.open(filepath, "r")
    local data = f.readLine()
    f.close()

    return data
end

-- Opens and replaces a file with a string.
local function rewrite_file(filepath, string)
    local f = fs.open(filepath, "w")
    f.write(string)
    f.close()
end

local facing_filepath = "turtle_command/facing.txt"

-- Updates the direction file when turning left
local function turnLeft()
    local dir = read_first_line(facing_filepath)
    local rewrite_with = ""
    if dir == "n" then
        rewrite_with = "w"
    elseif dir == "w" then
        rewrite_with = "s"
    elseif dir == "s" then
        rewrite_with = "e"
    elseif dir == "e" then
        rewrite_with = "n"
    end
    turtle.turnLeft()
    rewrite_file(facing_filepath, rewrite_with)
end

-- Updates the direction file when turning right
local function turnRight()
    local dir = read_first_line(facing_filepath)
    local rewrite_with = ""
    if dir == "n" then
        rewrite_with = "e"
    elseif dir == "e" then
        rewrite_with = "s"
    elseif dir == "s" then
        rewrite_with = "w"
    elseif dir == "w" then
        rewrite_with = "n"
    end
    turtle.turnRight()
    rewrite_file(facing_filepath, rewrite_with)

end

return {
    left = turnLeft,
    right = turnRight,
    read_first_line = read_first_line,
    rewrite_file = rewrite_file,
}