--!native
--!optimize 2

local Request = script.Parent.Request

local SYNC_DELAY = 1 / 8

local Stream = {}
Stream.__index = Stream

export type Stream = typeof(setmetatable(
	{} :: {
		_readBuf: buffer,
		_readBufFilled: number,
		_readBufPos: number,
		_readWaiting: { thread },

		_writeBuf: buffer,
		_writeBufFilled: number,
		_writeWaiting: { thread },

		_syncTime: number,
		_syncScheduled: boolean,
		_syncReading: boolean,
	},
	Stream
))

function Stream.new(): Stream
	local self = {
		_readBuf = buffer.create(4 * 1024 * 1024),
		_readBufFilled = 0,
		_readBufPos = 0,
		_readWaiting = {},

		_writeBuf = buffer.create(1024),
		_writeBufFilled = 0,
		_writeWaiting = {},

		_syncTime = 0,
		_syncScheduled = false,
		_syncReading = false,
	}

	setmetatable(self, Stream)
	return self
end

function Stream._scheduleSync(self: Stream)
	if self._syncScheduled then
		return
	end

	self._syncScheduled = true

	local nextSyncTime = self._syncTime + SYNC_DELAY
	task.delay(nextSyncTime - os.clock(), function()
		self:_sync()
	end)
end

function Stream._sync(self: Stream)
	self._syncScheduled = false
	self._syncTime = os.clock()

	local read = not self._syncReading and self._readBufPos >= self._readBufFilled
	if read then
		self._readBufFilled = 0
		self._readBufPos = 0
		self._syncReading = true
	end

	if not read and self._writeBufFilled == 0 then
		return
	end

	local payload = buffer.create(4 + self._writeBufFilled)

	if read then
		buffer.writeu32(payload, 0, buffer.len(self._readBuf) - self._readBufFilled)
	end

	buffer.copy(payload, 4, self._writeBuf, 0, self._writeBufFilled)
	self._writeBufFilled = 0

	for i, thread in self._writeWaiting do
		task.spawn(thread)
		self._writeWaiting[i] = nil
	end

	print(`>> {buffer.len(payload) - 4} bytes`)
	local response = Request:InvokeServer(payload)

	if read then
		print(`<< {buffer.len(response)} bytes`)

		buffer.copy(self._readBuf, 0, response)
		self._readBufFilled += buffer.len(response)

		for i, thread in self._readWaiting do
			task.spawn(thread)
			self._readWaiting[i] = nil
		end

		self._syncReading = false
	end
end

function Stream.read(self: Stream, target: buffer, targetOffset: number?, targetCount: number?): number
	local targetOffset = targetOffset or 0
	local targetCount = targetCount or buffer.len(target) - targetOffset

	local remaining = self._readBufFilled - self._readBufPos
	if remaining > 0 then
		local count = math.min(remaining, targetCount)
		buffer.copy(target, targetOffset, self._readBuf, self._readBufPos, count)
		self._readBufPos += count
		return count
	end

	table.insert(self._readWaiting, coroutine.running())
	self:_scheduleSync()
	coroutine.yield()

	return self:read(target, targetOffset, targetCount)
end

function Stream.readExact(self: Stream, target: buffer, targetOffset: number?, targetCount: number?)
	local targetOffset = targetOffset or 0
	local targetCount = targetCount or buffer.len(target) - targetOffset

	local read = 0
	while read < targetCount do
		read += self:read(target, targetOffset + read, targetCount - read)
	end
end

do
	local buf = buffer.create(4)
	local function implRead(count: number, f: (buf: buffer, offset: number) -> number): (self: Stream) -> number
		return function(self)
			local remaining = self._readBufFilled - self._readBufPos
			if remaining >= count then
				local n = f(self._readBuf, self._readBufPos)
				self._readBufPos += count
				return n
			end

			self:readExact(buf, 0, count)
			return f(buf, 0)
		end
	end

	Stream.readI8 = implRead(1, buffer.readi8)
	Stream.readU8 = implRead(1, buffer.readu8)
	Stream.readI16 = implRead(2, buffer.readi16)
	Stream.readU16 = implRead(2, buffer.readu16)
	Stream.readI32 = implRead(4, buffer.readi32)
	Stream.readU32 = implRead(4, buffer.readu32)
	Stream.readF32 = implRead(4, buffer.readf32)
	Stream.readF64 = implRead(8, buffer.readf64)
end

function Stream.readString(self: Stream, count: number)
	local remaining = self._readBufFilled - self._readBufPos
	if remaining >= count then
		local s = buffer.readstring(self._readBuf, self._readBufPos, count)
		self._readBufPos += count
		return s
	end

	local buf = buffer.create(count)
	self:readExact(buf)
	return buffer.tostring(buf)
end

function Stream.write(self: Stream, source: buffer, sourceOffset: number?, sourceCount: number?): number
	local sourceOffset = sourceOffset or 0
	local sourceCount = sourceCount or buffer.len(source) - sourceOffset

	local available = buffer.len(self._writeBuf) - self._writeBufFilled
	if available > 0 then
		local count = math.min(available, sourceCount)
		buffer.copy(self._writeBuf, self._writeBufFilled, source, sourceOffset, count)
		self._writeBufFilled += count

		self:_scheduleSync()
		return count
	end

	table.insert(self._writeWaiting, coroutine.running())
	self:_scheduleSync()
	coroutine.yield()

	return self:write(source, sourceOffset, sourceCount)
end

function Stream.writeAll(self: Stream, source: buffer, sourceOffset: number?, sourceCount: number?)
	local sourceOffset = sourceOffset or 0
	local sourceCount = sourceCount or buffer.len(source) - sourceOffset

	local wrote = 0
	while wrote < sourceCount do
		wrote += self:write(source, sourceOffset + wrote, sourceCount - wrote)
	end
end

do
	local buf = buffer.create(4)
	local function implWrite(
		count: number,
		f: (buf: buffer, offset: number, value: number) -> ()
	): (self: Stream, value: number) -> ()
		return function(self, value)
			local available = buffer.len(self._writeBuf) - self._writeBufFilled
			if available >= count then
				f(self._writeBuf, self._writeBufFilled, value)
				self._writeBufFilled += count
				self:_scheduleSync()
				return
			end

			f(buf, 0, value)
			self:writeAll(buf, 0, count)
		end
	end

	Stream.writeI8 = implWrite(1, buffer.writei8)
	Stream.writeU8 = implWrite(1, buffer.writeu8)
	Stream.writeI16 = implWrite(2, buffer.writei16)
	Stream.writeU16 = implWrite(2, buffer.writeu16)
	Stream.writeI32 = implWrite(4, buffer.writei32)
	Stream.writeU32 = implWrite(4, buffer.writeu32)
	Stream.writeF32 = implWrite(4, buffer.writef32)
	Stream.writeF64 = implWrite(8, buffer.writef64)
end

function Stream.writeString(self: Stream, s: string)
	local available = buffer.len(self._writeBuf) - self._writeBufFilled
	local count = #s

	if available >= count then
		buffer.writestring(self._writeBuf, self._writeBufFilled, s)
		self._writeBufFilled += count
		self:_scheduleSync()
		return
	end

	local buf = buffer.fromstring(s)
	self:writeAll(buf)
end

return table.freeze(Stream)
