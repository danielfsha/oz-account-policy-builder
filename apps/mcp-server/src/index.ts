/**
 * OZ Policy Builder — MCP Server
 *
 * Exposes the record → synthesize → emit → harness pipeline as MCP tools
 * that any MCP-compatible AI client (Claude, Cursor, etc.) can call.
 *
 * Auth: none required for local dev.
 * For production, add an auth layer via wrangler secrets (see .dev.vars.example).
 */

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { McpAgent } from "agents/mcp";
import { registerAllTools } from "./tools/register-tools";

// Props are empty for the policy builder — no user context needed
type Props = Record<string, never>;

export class MyMCP extends McpAgent<Env, Record<string, never>, Props> {
	server = new McpServer({
		name: "OZ Policy Builder",
		version: "0.1.0",
		instructions: `You are the OZ Policy Builder — an AI assistant that helps developers and users create OpenZeppelin smart account policies for Stellar (Soroban).

## Workflow
1. **record_transaction** — fetch a real Stellar tx by hash and extract a CallManifest
2. **synthesize_policy** — run the decision tree on the manifest to get a PolicySpec
3. **answer_clarification** — if the spec has open questions, resolve them here
4. **run_harness** — verify: original tx must PERMIT, 5 mutations must DENY
5. **emit_policy_crate** — generate the Rust crate (never auto-deployed)

## Available policy primitives (list_primitives for full details)
- spending_limit — rolling cap on outbound token transfers
- simple_threshold — M-of-N multisig
- weighted_threshold — weighted multisig
- time_window — once-per-period guard
- custom (generated) — slippage guard, subscription logic

## Rules
- Always show REVIEW.md after emit_policy_crate
- Never deploy automatically — user reviews code first
- Confirm spending caps and time windows with the user before emitting
`,
	});

	async init() {
		registerAllTools(this.server, this.env, this.props);
	}
}

// Export as a standard Worker fetch handler — no OAuth wrapping needed
export default {
	fetch: MyMCP.serve("/mcp"),
} satisfies ExportedHandler<Env>;
