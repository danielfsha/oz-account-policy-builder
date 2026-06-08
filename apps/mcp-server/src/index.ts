/**
 * OZ Policy Builder — Cloudflare Worker MCP Server (HTTP transport)
 *
 * For local Claude Desktop use, use src/stdio.ts instead (see CLAUDE.md).
 * This Worker is for remote/deployed access via Cloudflare.
 */

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { McpAgent } from "agents/mcp";
import { registerAllTools } from "./tools/register-tools";

type Props = Record<string, never>;

export class MyMCP extends McpAgent<Env, Record<string, never>, Props> {
	server = new McpServer({
		name: "OZ Policy Builder",
		version: "0.1.0",
	});

	async init() {
		registerAllTools(this.server, this.env, this.props);
	}
}

// Route all requests through the DO — McpAgent.serve handles /mcp and /sse
const handler = MyMCP.serve("/mcp");

export default {
	async fetch(request: Request, env: Env, ctx: ExecutionContext): Promise<Response> {
		const url = new URL(request.url);

		// Health check
		if (url.pathname === "/" && request.method === "GET") {
			return new Response(JSON.stringify({
				name: "OZ Policy Builder MCP",
				version: "0.1.0",
				endpoints: { mcp: "/mcp", sse: "/sse" },
				docs: "https://github.com/danielfsha/oz-account-policy-builder",
			}), { headers: { "Content-Type": "application/json" } });
		}

		// Reject OAuth discovery probes cleanly (mcp-remote tries these)
		if (url.pathname.startsWith("/.well-known/")) {
			return new Response("Not used — this server has no OAuth", { status: 404 });
		}

		// Route /mcp and /sse to the Durable Object
		return handler.fetch(request, env, ctx);
	},
} satisfies ExportedHandler<Env>;
