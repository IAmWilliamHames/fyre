//! # A Lightweight, Scriptable Rust HTTP Server
//!
//! Fyre combines the performance of a compiled Rust core with the
//! dynamic flexibility of Lua scripting for all endpoint logic.
//! This file contains the main server logic, configuration loading,
//! and the Lua pipeline execution.

use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex};

// Importing necessary mlua types.
use mlua::prelude::*; // Brings LuaTable, LuaFunction, etc. into scope
use mlua::{Error as LuaError, Lua}; // Only imports what is available in the root mlua module
use std::io::Cursor;
use std::path::Path;
use tiny_http::{Header, Response, Server, StatusCode};

/// A type alias for a thread-safe, shared map of routes.
///
/// The keys are the routes and the values are the paths to the Lua scripts that
/// handle them.
type RoutesMap = Arc<Mutex<HashMap<String, String>>>;

// --- Configuration ---
/// The default server address and port.
const DEFAULT_SERVER_ADDR: &str = "0.0.0.0:8000";
/// The directory where Lua handler scripts are stored.
const LUA_SCRIPTS_DIR: &str = "scripts";
/// The filename of the Lua configuration script.
const CONFIG_FILE: &str = "config.lua";

/// Initializes and runs the web server.
///
/// This is the main entry point for the application. It performs the following steps:
///
/// 1. **Initializes Routes:** A new, empty `RoutesMap` is created to store the
///    routing information.
///
/// 2. **Determines Server Address:** The server address is determined in the
///    following order of precedence:
///    - A command-line argument, if provided.
///    - The `SERVER_ADDR` global variable in `config.lua`, if set.
///    - The `DEFAULT_SERVER_ADDR` constant.
///
/// 3. **Loads Configuration:** The `load_lua_config` function is called to
///    execute the `config.lua` script, which populates the `RoutesMap`.
///
/// 4. **Starts Server:** The server is started on the determined address.
///
/// 5. **Enters Request Loop:** The server enters an infinite loop, processing
///    incoming requests. For each request, it looks up the route in the
///    `RoutesMap` and, if found, executes the corresponding Lua handler
///    script. If a route is not found, a 404 Not Found response is sent.
///
/// # Panics
///
/// This function will panic if it fails to lock the `RoutesMap` mutex.
///
/// # Errors
///
/// This function will return an error if:
/// - The Lua configuration file cannot be loaded.
/// - The server fails to start.
fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
  println!("INFO: Server starting up...");

  let routes: RoutesMap = Arc::new(Mutex::new(HashMap::new()));

  // --- Dynamic server address ---
  let mut server_addr = DEFAULT_SERVER_ADDR.to_string();

  if let Some(arg_addr) = std::env::args().nth(1) {
    server_addr = arg_addr;
    println!("INFO: Server address set by CLI argument: {}", server_addr);
  }

  match load_lua_config(routes.clone()) {
    Ok(lua_addr_option) => {
      println!("INFO: Successfully loaded routes from {}", CONFIG_FILE);
      if std::env::args().len() <= 1 {
        if let Some(addr) = lua_addr_option {
          server_addr = addr;
          println!("INFO: Server address set by config.lua: {}", server_addr);
        }
      }
    }
    Err(e) => {
      eprintln!("ERROR: Failed to load configuration: {}", e);
      return Err(e);
    }
  }

  println!(
    "INFO: Registered Routes: {:?}",
    routes.lock().unwrap().keys()
  );

  let server = Server::http(&server_addr).map_err(|e| format!("Could not start server: {}", e))?;
  println!("INFO: Server running at http://{}", server_addr);

  // Request Loop
  for mut request in server.incoming_requests() {
    let route = request.url().to_string();

    if let Some(script_path) = routes.lock().unwrap().get(&route).cloned() {
      println!("INFO: Request: {} -> Handler: {}", route, script_path);

      match execute_handler_pipeline(&mut request, &script_path) {
        Ok(response) => {
          if let Err(e) = request.respond(response) {
            eprintln!("ERROR: Error sending response: {}", e);
          }
        }
        Err(e) => {
          eprintln!("ERROR: Pipeline execution fatal error for {}: {}", route, e);
          let err_response =
            Response::from_string(format!("Server Error: {}", e)).with_status_code(500);
          if let Err(e) = request.respond(err_response) {
            eprintln!("ERROR: Error sending error response: {}", e);
          }
        }
      }
    } else {
      eprintln!("WARN: 404 Not Found: {}", route);
      let not_found = Response::from_string("404 Not Found").with_status_code(404);
      if let Err(e) = request.respond(not_found) {
        eprintln!("ERROR: Error sending 404 response: {}", e);
      }
    }
  }

  Ok(())
}

/// Loads and executes the Lua configuration script.
///
/// This function is responsible for setting up the Lua environment and running the
/// `config.lua` script. It creates a new `Lua` instance and exposes two
/// functions to the script:
///
/// - `router.add(path, script)`: Registers a new route. `path` is the URL path
///   and `script` is the filename of the Lua handler script in the
///   `LUA_SCRIPTS_DIR` directory.
/// - `router.set_addr(address)`: Sets the server address. This is currently a
///   noop and is only logged. The server address is actually set by the
///   `SERVER_ADDR` global variable.
///
/// The function also checks for a global variable named `SERVER_ADDR` in the
/// Lua script. If it's found, its value is returned and used as the server
/// address.
///
/// # Arguments
///
/// * `routes_arc` - A thread-safe, shared `RoutesMap` that will be populated by
///   the `router.add` function in the Lua script.
///
/// # Errors
///
/// This function will return an error if:
/// - The `config.lua` file cannot be read.
/// - The Lua script fails to execute.
/// - It fails to lock the `RoutesMap` mutex.
fn load_lua_config(
  routes_arc: RoutesMap,
) -> std::result::Result<Option<String>, Box<dyn std::error::Error>> {
  let lua = Lua::new();
  let globals = lua.globals();

  let mut configured_addr: Option<String> = None;

  let router_table = lua.create_table()?;
  router_table.set(
    "add",
    lua.create_function(move |_, (path, script): (String, String)| {
      let mut routes = routes_arc
        .lock()
        .map_err(|_| LuaError::external("Failed to lock routes"))?;

      let full_script_path = format!("{}/{}", LUA_SCRIPTS_DIR, script);
      if !Path::new(&full_script_path).exists() {
        return Err(LuaError::external(format!(
          "Handler script not found: {}",
          full_script_path
        )));
      }

      println!("INFO: Registering route: {} -> {}", path, full_script_path);
      routes.insert(path, full_script_path);
      Ok(())
    })?,
  )?;

  router_table.set(
    "set_addr",
    lua.create_function(|_, addr: String| {
      println!("INFO: 'router.set_addr' called with: {}. The Rust host will resolve this after config execution.", addr);
      Ok(())
    })?
  )?;

  globals.set("router", router_table)?;

  let config_code = fs::read_to_string(CONFIG_FILE)?;
  lua.load(&config_code).set_name(CONFIG_FILE).exec()?;

  if let Ok(lua_addr) = globals.get::<String>("SERVER_ADDR") {
    configured_addr = Some(lua_addr);
  }

  Ok(configured_addr)
}

// Executes the three-stage handler pipeline: MIDDLEWARE -> HANDLER (conditional) -> RESPONSE HOOK.
/// Executes a Lua handler script and its associated middleware.
///
/// This function orchestrates the execution of a Lua script in a three-stage
/// pipeline:
///
/// 1.  **`middleware`:** If the script returns a table containing a `middleware`
///     function, it is executed first. This function can inspect the request and
///     modify the response. If it sets the response status to anything other
///     than 200, the main `handler` is skipped.
///
/// 2.  **`handler`:** If the script returns a table containing a `handler` function
///     and the middleware did not intercept the request, this function is
///     executed. It is responsible for the main request processing logic.
/// 3.  **`response_hook`:** If the script returns a table containing a
///     `response_hook` function, it is always executed after the `handler`
///     (or after the `middleware` if the handler was skipped). This can be used
///     for final modifications to the response, such as adding headers or
///     logging.
///
/// The function sets up two global tables for the Lua script:
///
/// - `request`: An immutable table containing request data (method, path, body,
///   headers).
/// - `response`: A mutable table that the script can modify to set the response
///   status, body, and headers.
///
/// # Arguments
///
/// * `req` - A mutable reference to the `tiny_http::Request`.
/// * `script_path` - The path to the Lua handler script to execute.
///
/// # Errors
///
/// This function will return a `LuaError` if:
/// - The handler script cannot be read.
/// - The handler script fails to return a table.
/// - The main `handler` function in the script returns an error.
/// - There are issues getting or setting values in the `response` table.
fn execute_handler_pipeline(
  req: &mut tiny_http::Request,
  script_path: &str,
) -> std::result::Result<Response<std::io::Cursor<Vec<u8>>>, LuaError> {
  let lua = Lua::new();

  // --- 1. Prepare Data Tables ---
  let mut body_bytes = Vec::new();
  let _ = req
    .as_reader()
    .read_to_end(&mut body_bytes)
    .map_err(|e| LuaError::external(format!("Failed to read request body: {}", e)))?;
  let body_string = String::from_utf8(body_bytes).unwrap_or_default();

  // Request Table (Immutable Input)
  let req_table = lua.create_table()?;
  req_table.set("method", req.method().as_str())?;
  req_table.set("path", req.url())?;
  req_table.set("body", body_string)?;
  let headers_table = lua.create_table()?;
  for header in req.headers() {
    headers_table.set(header.field.as_str().to_string(), header.value.to_string())?;
  }
  req_table.set("headers", headers_table)?;

  // Response Table (Mutable Output/State)
  let res_table = lua.create_table()?;
  res_table.set("status", 200i32)?;
  res_table.set("body", String::new())?;
  res_table.set("headers", lua.create_table()?)?;

  // Expose tables as globals for Lua
  let globals = lua.globals();
  globals.set("request", req_table.clone())?;
  globals.set("response", res_table.clone())?;

  // --- 2. Load the Route Script (Modular Module Execution) ---
  let script_code = fs::read_to_string(script_path).map_err(|e| {
    LuaError::external(format!(
      "Failed to read handler script {}: {}",
      script_path, e
    ))
  })?;

  // Execute script and capture its returned value (the module table)
  let module_table = lua
    .load(&script_code)
    .set_name(script_path)
    .eval::<LuaTable>() // Expects the Lua script to `return { ... }`
    .map_err(|e| {
      LuaError::external(format!("Handler script failed to return a table: {}", e))
    })?;

    // --- 3. Execute Pipeline ---

    // A. BEFORE Middleware: Get 'middleware' function
    if let Ok(before) = module_table.get::<LuaFunction>("middleware") {
      if let Err(e) = before.call::<()>((req_table.clone(), res_table.clone())) {
        eprintln!("WARN: Middleware error (before handler): {}", e);
      }
    }

    // Check if BEFORE middleware intercepted (status != 200)
    let current_status: i32 = res_table.get("status").unwrap_or(200);

    if current_status == 200 {
      // B. MAIN HANDLER: Get 'handler' function
      match module_table.get::<LuaFunction>("handler") {
        Ok(handler) => {
          if let Err(e) = handler.call::<()>((req_table.clone(), res_table.clone())) {
            return Err(e); // Propagate handler failure
          }
        }
        Err(_) => {
          println!(
            "WARN: No 'handler' function found in {}. Response might be empty.",
              script_path
            );
          }
        }
    } else {
      println!(
        "INFO: Request intercepted by middleware (Status: {})",
        current_status
      );
    }

    // C. AFTER Middleware: Get 'response_hook' function
    if let Ok(after) = module_table.get::<LuaFunction>("response_hook") {
      if let Err(e) = after.call::<()>((req_table.clone(), res_table.clone())) {
        eprintln!("WARN: Response hook error (after handler): {}", e);
      }
    }

    // --- 4. Finalize Response ---
    let final_status: i32 = res_table.get("status").unwrap_or(500);
    let body_string: String = res_table.get("body").map_err(|e| {
      LuaError::external(format!("Failed to get body from response table: {}", e))
    })?;

    let mut response = Response::new(
      StatusCode(final_status as u16),
      vec![],
      Cursor::new(body_string.into_bytes()),
      None,
      None,
    );

    let headers_table: LuaTable = res_table.get("headers")?;
    for pair in headers_table.pairs::<String, String>() {
      let (key, value) = pair?;
      if let Ok(header) = Header::from_bytes(key.as_bytes(), value.as_bytes()) {
        response.add_header(header);
      } else {
          eprintln!("WARN: Invalid header skipped: {}: {}", key, value);
      }
    }

    Ok(response)
}
