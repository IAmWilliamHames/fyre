-- Set the server address here. 
-- The value below will be used unless a CLI argument overrides it.
SERVER_ADDR = "localhost:9000"

-- Maps incoming URL paths to specific handler script files.
-- router.add(path, handler_script_filename)

-- Public endpoint demo
router.add("/", "default_api.lua")

-- Protected endpoint demo
router.add("/api/users", "user_api.lua")