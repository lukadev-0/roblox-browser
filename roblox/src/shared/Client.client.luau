local Browser = require(script.Parent.Browser)

local ui = workspace.Screen.Browser
local urlInput = ui.Background.Bar.URLInput
local loadButton = ui.Background.Bar.LoadButton

local browser = Browser.new(ui.Background.ViewportContainer)

loadButton.Activated:Connect(function()
	browser:load(urlInput.Text)
end)
