--!strict
-- Test service: lives only on the server (name ends in "Server").

local CombatServiceServer = {}

function CombatServiceServer.Start()
	print("CombatServiceServer started")
end

return CombatServiceServer
