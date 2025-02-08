--!native
--!optimize 2

local AssetService = game:GetService("AssetService")
local GuiService = game:GetService("GuiService")

local Stream = require(script.Parent.Stream)
local protocol = require(script.Parent.Protocol)

local CHUNK_SIZE = 128

local lastWait = os.clock()
local function waitIfNeeded()
	if os.clock() - lastWait >= 1 / 40 then
		task.wait()
		lastWait = os.clock()
	end
end

local Browser = {}
Browser.__index = Browser

export type Browser = typeof(setmetatable(
	{} :: {
		stream: Stream.Stream,
		viewport: Frame,
		width: number,
		height: number,

		onFocus: (() -> ())?,
		onUnfocus: (() -> ())?,

		_aspectRatioConstraint: UIAspectRatioConstraint,
		_images: { ImageLabel },
		_editableImages: { EditableImage },
		_ready: boolean,

		_focused: boolean,
	},
	Browser
))

function Browser.new(viewportContainer: Instance): Browser
	local self = {
		stream = Stream.new(),
		width = 0,
		height = 0,

		_focused = false,
		_focusSignals = {},
		_ready = false,
	}

	self.viewport = Instance.new("Frame")
	self.viewport.Name = "BrowserViewport"
	self.viewport.Size = UDim2.fromScale(1, 1)
	self.viewport.BackgroundTransparency = 1
	self.viewport.Position = UDim2.fromScale(0.5, 0.5)
	self.viewport.AnchorPoint = Vector2.new(0.5, 0.5)

	self._aspectRatioConstraint = Instance.new("UIAspectRatioConstraint")
	self._aspectRatioConstraint.Parent = self.viewport

	self._images = {}
	self._images[1] = Instance.new("ImageLabel")
	self._images[1].Name = "Image"
	self._images[1].Size = UDim2.fromScale(1, 1)
	self._images[1].BackgroundTransparency = 1
	self._images[1].Parent = self.viewport

	self._editableImages = {}

	self.viewport.Parent = viewportContainer

	setmetatable(self, Browser)

	task.spawn(function()
		local pixelsLen = CHUNK_SIZE * CHUNK_SIZE * 4
		local pixels = table.create(pixelsLen)

		while true do
			local command = self:read()
			waitIfNeeded()

			print(`<<<< command {command.kind}`)

			if command.kind == "Resize" then
				self.width = command.width
				self.height = command.height

				if not self._editableImages[1] then
					self._editableImages[1] = AssetService:CreateEditableImage({
						Size = Vector2.new(self.width, self.height),
					})
					self._images[1].ImageContent = Content.fromObject(self._editableImages[1])
				end

				self._aspectRatioConstraint.AspectRatio = self.width / self.height
				self._ready = true
			elseif not self._ready then
				continue
			elseif command.kind == "ChunkData" then
				local offsetX = command.chunkPos.X * CHUNK_SIZE
				local offsetY = command.chunkPos.Y * CHUNK_SIZE
				local chunkWidth = math.min(CHUNK_SIZE, self.width - offsetX)
				local chunkHeight = math.min(CHUNK_SIZE, self.height - offsetY)

				local len = chunkWidth * chunkHeight * 4

				self._editableImages[1]:WritePixelsBuffer(
					Vector2.new(offsetX, offsetY),
					Vector2.new(chunkWidth, chunkHeight),
					command.data
				)
			end
		end
	end)

	self.viewport.InputChanged:Connect(function(input)
		if input.UserInputType == Enum.UserInputType.MouseMovement then
			local inset = GuiService:GetGuiInset()
			local globalX = input.Position.X + inset.X
			local globalY = input.Position.Y + inset.Y

			print(globalX, globalY)
			local x =
				math.floor((globalX - self.viewport.AbsolutePosition.X) / self.viewport.AbsoluteSize.X * self.width)
			local y =
				math.floor((globalY - self.viewport.AbsolutePosition.Y) / self.viewport.AbsoluteSize.Y * self.height)

			print(x, y)

			self:dispatch({
				kind = "Mouse",
				x = x,
				y = y,
				event = {
					kind = "move",
				},
			})
		end
	end)

	self.viewport.InputBegan:Connect(function(input)
		if
			input.UserInputType == Enum.UserInputType.MouseButton1
			or input.UserInputType == Enum.UserInputType.MouseButton2
		then
			local inset = GuiService:GetGuiInset()
			local globalX = input.Position.X + inset.X
			local globalY = input.Position.Y + inset.Y

			local x =
				math.floor((globalX - self.viewport.AbsolutePosition.X) / self.viewport.AbsoluteSize.X * self.width)
			local y =
				math.floor((globalY - self.viewport.AbsolutePosition.Y) / self.viewport.AbsoluteSize.Y * self.height)

			self:dispatch({
				kind = "Mouse",
				x = x,
				y = y,
				event = {
					kind = "pressed",
					button = if input.UserInputType == Enum.UserInputType.MouseButton1 then "left" else "right",
				},
			})
		end
	end)

	self.viewport.InputEnded:Connect(function(input)
		if
			input.UserInputType == Enum.UserInputType.MouseButton1
			or input.UserInputType == Enum.UserInputType.MouseButton2
		then
			local inset = GuiService:GetGuiInset()
			local globalX = input.Position.X + inset.X
			local globalY = input.Position.Y + inset.Y

			local x =
				math.floor((globalX - self.viewport.AbsolutePosition.X) / self.viewport.AbsoluteSize.X * self.width)
			local y =
				math.floor((globalY - self.viewport.AbsolutePosition.Y) / self.viewport.AbsoluteSize.Y * self.height)

			self:dispatch({
				kind = "Mouse",
				x = x,
				y = y,
				event = {
					kind = "released",
					button = if input.UserInputType == Enum.UserInputType.MouseButton1 then "left" else "right",
				},
			})
		end
	end)

	self:dispatch({ kind = "Reset" })

	return self
end

function Browser.dispatch(self: Browser, command: protocol.ClientCommand): ()
	print(`>>>> command {command.kind}`)
	protocol.ClientCommand.write(self.stream, command)
end

function Browser.read(self: Browser): protocol.ServerCommand
	return protocol.ServerCommand.read(self.stream)
end

function Browser.load(self: Browser, url: string): ()
	self:dispatch({
		kind = "Load",
		url = url,
	})
end

return table.freeze(Browser)
