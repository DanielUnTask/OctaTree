--!strict
-- Test service: lives only on the client (name ends in "Client").

local CombatServiceClient = {}

function CombatServiceClient.Start()
	print("CombatServiceClient started")
end

return CombatServiceClient
