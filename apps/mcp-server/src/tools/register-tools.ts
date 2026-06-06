import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { Props } from "../types";
import { registerDatabaseTools } from "./database-tools";

/**
 * Register all MCP tools based on user permissions
 */
export function registerAllTools(server: McpServer, env: Env, props: Props) {
	// Register database tools
	registerDatabaseTools(server, env, props);
	
	// Future tools can be registered here
	// registerOtherTools(server, env, props);
}