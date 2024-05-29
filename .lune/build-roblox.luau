local fs = require("@lune/fs")
local process = require("@lune/process")
local stdio = require("@lune/stdio")

if fs.isDir("roblox/build") then
	fs.removeDir("roblox/build")
end
fs.writeDir("roblox/build")

local result = process.spawn("rojo", { "build", "-o", "roblox/build/browser.rbxl" }, {
	shell = true,
	stdio = "forward",
})

if not result.ok then
	stdio.ewrite(`\n{stdio.style("bold")}{stdio.color("red")}build failed{stdio.style("reset")}\n`)
	process.exit(result.code)
end

stdio.write(`\nbuilt roblox project to `)
stdio.write(`{stdio.style("bold")}roblox/build/browser.rbxl{stdio.style("reset")}\n`)
