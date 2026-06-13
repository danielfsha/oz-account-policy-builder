import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { registerPolicyTools } from "./policy-tools";

export type Props = {
  login?: string;
  name?: string;
  email?: string;
  accessToken?: string;
};

export function registerAllTools(server: McpServer, env: Env, props: Props) {
  registerPolicyTools(server, env, props);
}
