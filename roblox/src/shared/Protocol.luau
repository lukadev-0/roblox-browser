--!native
--!optimize 2

local Stream = require(script.Parent.Stream)

local protocol = {}

local ClientCommand = {}

export type ChunkPos = Vector3

local ChunkPos = {}

function ChunkPos.read(stream: Stream.Stream): ChunkPos
	local x = stream:readU8()
	local y = stream:readU8()
	return Vector3.new(x, y)
end

protocol.ChunkPos = table.freeze(ChunkPos)

export type ClientCommand = {
	kind: "Reset",
} | {
	kind: "Load",
	url: string,
} | {
	kind: "Mouse",
	x: number,
	y: number,
	event: {
		kind: "move",
	} | {
		kind: "pressed",
		button: "left" | "right",
	} | {
		kind: "released",
		button: "left" | "right",
	},
}

function ClientCommand.write(stream: Stream.Stream, command: ClientCommand)
	if command.kind == "Reset" then
		stream:writeU8(0)
	elseif command.kind == "Load" then
		stream:writeU8(1)
		stream:writeU32(#command.url)
		stream:writeString(command.url)
	elseif command.kind == "Mouse" then
		stream:writeU8(2)
		stream:writeU32(command.x)
		stream:writeU32(command.y)

		if command.event.kind == "move" then
			stream:writeU8(0)
		elseif command.event.kind == "pressed" then
			if command.event.button == "left" then
				stream:writeU8(1)
			else
				stream:writeU8(2)
			end
		elseif command.event.kind == "released" then
			if command.event.button == "left" then
				stream:writeU8(3)
			else
				stream:writeU8(4)
			end
		end
	end
end

protocol.ClientCommand = table.freeze(ClientCommand)

local ServerCommand = {}

export type ServerCommand = {
	kind: "Resize",
	width: number,
	height: number,
} | {
	kind: "ChunkData",
	chunkPos: ChunkPos,
	data: buffer,
}

function ServerCommand.read(stream: Stream.Stream): ServerCommand
	local kind = stream:readU8()
	if kind == 0 then
		local width = stream:readU32()
		local height = stream:readU32()

		return { kind = "Resize", width = width, height = height }
	elseif kind == 1 then
		local chunkPos = ChunkPos.read(stream)
		local len = stream:readU32()

		local buf = buffer.create(len)
		stream:readExact(buf)

		return { kind = "ChunkData", chunkPos = chunkPos, data = buf }
	end

	error(`i no know what a {kind} is`)
end

protocol.ServerCommand = table.freeze(ServerCommand)

return table.freeze(protocol)
