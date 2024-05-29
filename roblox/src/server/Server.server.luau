--!native
--!optimize 2

local HttpService = game:GetService("HttpService")
local ReplicatedStorage = game:GetService("ReplicatedStorage")

local URL = "http://localhost:3000/"

ReplicatedStorage.Request.OnServerInvoke = function(_player, data: buffer)
	return buffer.fromstring(HttpService:PostAsync(URL, buffer.tostring(data)))
end
