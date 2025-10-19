-- Define local functions for the pipeline stages

-- 1. MIDDLEWARE (The 'before' stage)
local function default_middleware(request, response)
  print("[/]: MIDDLEWARE - Request received.")
    
  -- Example: Set a default header before the main handler runs
  response.headers["X-Request-Path"] = request.path
end

-- 2. HANDLER (The core logic stage)
local function default_handler(request, response)
  print("[/]: HANDLER - Processing request.")
    
  response.status = 200
  response.headers["Content-Type"] = "application/json"
    
  local body_content = string.format([[
    {
      "endpoint": "/",
      "method": "%s",
      "message": "Public access granted. Welcome to the modular router!"
    }
  ]], request.method)
    
  response.body = body_content
end

-- 3. RESPONSE HOOK (The 'after' stage)
local function default_response_hook(request, response)
    print("[/]: RESPONSE HOOK - Adding final header.")
    response.headers["X-Response-ID"] = "ABC-123"
end

-- Return the public interface table for the Rust host
return { 
  middleware    = default_middleware,   -- The request is processed by 'middleware' first (optional)
  handler       = default_handler,      -- The core logic is executed by 'handler' (required for content generation)
  response_hook = default_response_hook -- The response is processed by 'response_hook' last (optional)
}