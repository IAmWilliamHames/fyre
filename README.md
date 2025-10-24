# A Lightweight, Scriptable Rust HTTP Server  

Fyre combines the performance of a compiled Rust core with the dynamic flexibility of Lua scripting for all your endpoint logic. Define your API behavior in simple Lua files, change them on the fly, and never recompile your server.  

## What Is This?  

Fyre is a minimal Rust HTTP server (using `tiny_http`) that doesn't hardcode any routes. Instead, it reads a `config.lua` file to map URL paths directly to individual Lua scripts.  

When a request comes in, Fyre executes the corresponding script, giving it full control over the response. This makes it incredibly easy to build, prototype, and maintain a web service where all the logic lives in clean, separate Lua files.  

## Features  

- **High Performannce**: Built on a lightweight Rust core using `tiny_http`.
- **Extremely Flexible**: All endpoint logic is written in Lua. Add or change endpoints without any server recompilation.
- **Modular by Design**: Each route maps to its own script, keeping your code clean and concerns separated.
- **Powerful Middleware Pipeline**: Each script can implement a 3-stage pipeline (`middleware`, `handler`, `response_hook`) for handling auth, logging, and core logic with precision.
- **Simple Configuration**: A single `config.lua` file manages all your routes.

## How It Works

The server is built on two core concepts: the router configuration and the 3-stage handler pipeline.  

### 1. The Configuration (`config.lua`)

At startup, the server loads `config.lua`. Your only job here is to define the `SERVER_ADDR` and map your routes using `router.add()`.  

The Rust host reads this file and maps each path to its handler script in the `scripts/` directory.  

```lua
-- File: config.lua

-- Set the server address
SEVER_ADDR = "localhost:9900"

-- Maps incoming URL paths to specific handler script files
-- router.add(path, handler_script_filename)

-- Endpoint demo
router.add("/", "index.lua")
```

### 2. The 3-Stage Lua Handler Pipeline

When Fyre receives a request, it executes the corresponding Lua script (`scripts/index.lua`) and looks for a returned table containing three specific functions:  

1. `middleware(request, response)`: (Optional) This function runs **first**. It can "intercept" a request (e.g., by setting `response.status = 401`) to prvent the main handler from running.
2. `handler(request, response)`: (Required) This is your main logic. It only runs if the `middleware` stage did not intercept the request (i.e., `response/status` is still 200). This is where you fetch data and build your successful response body.
3. `response_hook(request, response)`: (Optional) This function runs **last**, no matter what. It is perfect for logging the *final* response status, adding final headers, or resource cleanup.  

The `request` table is read-only, while the `response` table is mutable, allowing each stage to build upon the previous one.  

## Examples

Here are two examples demonstrating the pipeline.  

### Example 1: A Public Endpoint (`/`)

A simple endpoint that uses all three stages for logging and setting headers.  
```lua
-- 1. Middleware (The 'before' stage)
local function my_middleware(request, response)
  print("[/]: Middleware - Request received.")
  response.headers["X-Request-Path"] = request.path
end

-- 2. Handler (The core logic stage)
local function my_handler(request, response)
  response.status = 200
  response.headers["Content-Type"] = "application/json"
  response.body = string.format([[
    {
      "message": "Public access granted. Welcome to the modular router."
    }
  ]], request.method)
end

-- 2. Response Hook (The 'after' stage)
local function my_response_hook(request, response)
  print("[/]: Response Hook - Adding final header")
  response.headers["X-Response-ID"] = "ABC-123"
end

-- Return the public interface table for the Rust host
return {
  middleware    = my_middleware,
  handler       = my_handler,
  response_hook = my_response_hook
}
```
### Example 2: The Protected Endpoint  

Shows how to use the `middleware` stage to perform authorization and **intercept** a request, preventing the main handler from even running.  
```lua
-- 1. Middleware (Auth check & interception)
local function auth_middleware(request, response)
  print("[/auth]: Middleware - Checking Authorization")

  local auth_header = request.headers["Authorization"]

  if auth_header ~= "Bearer secret-token" then
    print("[/auth]: Auth FAILED (401)")
    response.status = 401 -- This intercepts the request
    response.headers["Content-Type"] = "application/json"
    response.body = [[{"error": "Unauthorized"}]]
  else
    print("[/auth]: Auth SUCCESS")
  end
end

-- 2. Handler (Only runs if middleware passes)
local function my_handler(request, response)
  print("[/auth]: Handler - Authorized data access.")

  response.status = 200
  response.headers["Content-Type"] = "application/json"
  response.body = [[
    {
      "sensitive-data": "Sensitive content only for token holders."
    }
  ]]
end

-- Return the public interface table
return {
  middleware = auth_middleware,
  handler    = my_handler
}
```

If you call this endpoint without the correct token, the `auth_middleware` sets the status to 401. The Rust core sees this and **skips** the `my_handler`, immediately sending the `{"error": "Unauthorized"}` response.  

## How to Run

1. Ensure you have Rust and Cargo installed.
2. Build the server:
```bash
cargo build --release
```
3. Run the server:
```bash
./target/release/scriptable-server
```
4. The server will start using the address in the `config.lua` (e.g., `localhost:9900`). You can also override the configured address with a command-line argument:
```bash
./target/release/scriptable-server 0.0.0.0:80
```







