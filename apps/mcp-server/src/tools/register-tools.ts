import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { registerPolicyTools } from "./policy-tools";

export function registerAllTools(server: McpServer, env: Env, props: Record<string, never>) {
  registerPolicyTools(server, env, props);
}
