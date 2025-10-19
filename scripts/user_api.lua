-- 1. MIDDLEWARE (The 'before' stage, handling Auth Check & Interception)
local function auth_middleware(request, response)
  print("[/api/users]: MIDDLEWARE - Checking Authorization.")
    
  local auth_header = request.headers["Authorization"]
    
  if auth_header ~= "Bearer secret-token" then
    print("[/api/users]: Auth FAILED (401)")
    response.status = 401 
    response.headers["Content-Type"] = "application/json"
    response.body = [[{"error": "Unauthorized"}]]
    -- By setting status to 401, we intercept the request and skip the main handler.
  else
    print("[/api/users]: Auth SUCCESS.")
  end
end

-- 2. HANDLER (The core data access stage)
local function user_data_handler(request, response)
  print("[/api/users]: HANDLER - Authorized data access.")
    
  -- If we reach here, response.status is still 200 (from the check in Rust)
  response.status = 200
  response.headers["Content-Type"] = "application/json"

  local body_content = [[
    {
      "user_data": "Sensitive content only for token holders.",
      "method": "]] .. request.method .. [["
    }
  ]]
    
  response.body = body_content
end

-- 3. RESPONSE HOOK (The 'after' stage)
local function final_user_hook(request, response)
  print("[/api/users]: RESPONSE HOOK - Final status: " .. response.status)
end

-- Return the public interface table for the Rust host
return {
  middleware = auth_middleware,   -- Handles authorization and request interception
  handler = user_data_handler,    -- Executes the main logic if not intercepted
  response_hook = final_user_hook -- Runs cleanup or logging regardless of success/failure
}