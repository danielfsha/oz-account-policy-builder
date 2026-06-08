#!/usr/bin/env node
/**
 * OZ Policy Builder — stdio MCP server
 *
 * This is the entry point for Claude Desktop and other local MCP clients.
 * It uses the stdio transport — no HTTP server, no mcp-remote, no wrangler needed.
 *
 * Claude Desktop config (~\AppData\Roaming\Claude\claude_desktop_config.json):
 * {
 *   "mcpServers": {
 *     "oz-policy-builder": {
 *       "command": "node",
 *       "args": ["C:/path/to/oz-account-policy-builder/apps/mcp-server/dist/stdio.js"]
 *     }
 *   }
 * }
 *
 * Or with npx (after npm run build):
 * {
 *   "mcpServers": {
 *     "oz-policy-builder": {
 *       "command": "npx",
 *       "args": ["tsx", "C:/path/to/oz-account-policy-builder/apps/mcp-server/src/stdio.ts"]
 *     }
 *   }
 * }
 */

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  RecordTransactionSchema,
  SynthesizePolicySchema,
  EmitPolicySchema,
  RunHarnessSchema,
  ListPrimitivesSchema,
  AnswerClarificationSchema,
  createSuccessResponse,
  createErrorResponse,
} from "./types.js";
import {
  fetchTransactionFromHorizon,
  buildCallManifest,
  synthesizePolicy,
  emitPolicyCrate,
  runHarness,
  OZ_PRIMITIVES,
  applyConstraintOverride,
} from "./pipeline/index.js";

const server = new McpServer({
  name: "OZ Policy Builder",
  version: "0.1.0",
  instructions: `You are the OZ Policy Builder — helps craft OpenZeppelin smart account policies for Stellar.

Workflow: record_transaction → synthesize_policy → answer_clarification (if needed) → run_harness → emit_policy_crate

Rules: always show REVIEW.md after emit, never auto-deploy, confirm caps with user first.`,
});

// ── Tool: record_transaction ─────────────────────────────────────────────────
server.tool(
  "record_transaction",
  "Fetch a Stellar transaction by hash from Horizon and extract a structured CallManifest for policy synthesis.",
  RecordTransactionSchema,
  async (args: { tx_hash: string; network: "mainnet" | "testnet" | "futurenet"; invoking_account?: string }) => {
    try {
      const raw = await fetchTransactionFromHorizon(args.tx_hash, args.network as any);
      const manifest = buildCallManifest(raw, args.invoking_account);
      return createSuccessResponse(`Recorded tx \`${args.tx_hash.slice(0, 12)}…\` on ${args.network}`, { manifest });
    } catch (err: any) {
      return createErrorResponse("Failed to record transaction", { message: err.message });
    }
  }
);

// ── Tool: synthesize_policy ──────────────────────────────────────────────────
server.tool(
  "synthesize_policy",
  "Synthesize an OZ account PolicySpec from a CallManifest. Returns context rule + policy layers + any clarification questions.",
  SynthesizePolicySchema,
  async (args: { manifest_json: string; amount_cap?: string; time_window_seconds?: number; lifetime_seconds?: number; max_slippage_percent?: number }) => {
    try {
      const manifest = JSON.parse(args.manifest_json);
      const spec = synthesizePolicy(manifest, {
        amount_cap: args.amount_cap ? BigInt(args.amount_cap) : undefined,
        time_window_seconds: args.time_window_seconds,
        lifetime_seconds: args.lifetime_seconds,
        allow_slippage_percent: args.max_slippage_percent,
      });
      const hasClarifications = spec.clarifications_needed?.length > 0;
      const msg = hasClarifications
        ? `Synthesized — ${spec.clarifications_needed.length} clarification(s) needed`
        : `Synthesized — ${spec.policies.length} layers, mode: ${spec.composition_mode}`;
      return createSuccessResponse(msg, { spec, clarifications: spec.clarifications_needed });
    } catch (err: any) {
      return createErrorResponse("Synthesis failed", { message: err.message });
    }
  }
);

// ── Tool: answer_clarification ───────────────────────────────────────────────
server.tool(
  "answer_clarification",
  "Resolve a pending clarification in a PolicySpec (e.g. amount_cap, time_window_seconds).",
  AnswerClarificationSchema,
  async (args: { spec_json: string; field: string; answer: string }) => {
    try {
      const spec = JSON.parse(args.spec_json);
      const updated = applyConstraintOverride(spec, args.field, args.answer);
      const remaining = updated.clarifications_needed?.length ?? 0;
      return createSuccessResponse(
        remaining === 0 ? "All clarifications resolved — ready to emit" : `${remaining} question(s) remaining`,
        { spec: updated, remaining_clarifications: updated.clarifications_needed }
      );
    } catch (err: any) {
      return createErrorResponse("Failed to apply clarification", { message: err.message });
    }
  }
);

// ── Tool: run_harness ────────────────────────────────────────────────────────
server.tool(
  "run_harness",
  "Run permit/deny simulation: original tx must PASS, 5 mutation cases must FAIL.",
  RunHarnessSchema,
  async (args: { spec_json: string; manifest_json: string }) => {
    try {
      const spec = JSON.parse(args.spec_json);
      const manifest = JSON.parse(args.manifest_json);
      const report = runHarness(spec, manifest);
      return createSuccessResponse(
        report.passed ? "ALL CASES PASSED — safe to emit" : "HARNESS FAILED — tighten constraints",
        { passed: report.passed, permit: report.permit_result, deny_cases: report.deny_results, report: report.report }
      );
    } catch (err: any) {
      return createErrorResponse("Harness failed to run", { message: err.message });
    }
  }
);

// ── Tool: emit_policy_crate ──────────────────────────────────────────────────
server.tool(
  "emit_policy_crate",
  "Generate a compilable Rust Soroban policy crate from a PolicySpec. Returns file contents as strings. NEVER auto-deploys.",
  EmitPolicySchema,
  async (args: { spec_json: string; output_dir?: string }) => {
    try {
      const spec = JSON.parse(args.spec_json);
      if (spec.clarifications_needed?.length > 0) {
        return createErrorResponse("Unresolved clarifications — run answer_clarification first", {
          pending: spec.clarifications_needed.map((c: any) => c.question),
        });
      }
      const emitted = emitPolicyCrate(spec, args.output_dir);
      return createSuccessResponse(
        `Generated \`${emitted.summary.crate_name}\` (${emitted.summary.layer_count} layers, ${emitted.summary.mode} mode)`,
        { crate_path: emitted.crate_path, files: emitted.files, review_md: emitted.review_md, summary: emitted.summary }
      );
    } catch (err: any) {
      return createErrorResponse("Emit failed", { message: err.message });
    }
  }
);

// ── Tool: list_primitives ────────────────────────────────────────────────────
server.tool(
  "list_primitives",
  "List all available OpenZeppelin account policy primitives.",
  ListPrimitivesSchema,
  async () => createSuccessResponse("OZ Policy Primitives", {
    primitives: OZ_PRIMITIVES,
    crate: "stellar-accounts = \"=0.7.1\"",
    docs: "https://docs.openzeppelin.com/stellar-contracts/accounts/policies",
  })
);

// ── Start ────────────────────────────────────────────────────────────────────
const transport = new StdioServerTransport();
await server.connect(transport);
// Log to stderr only — stdout is reserved for MCP protocol messages
console.error("[oz-policy-builder] stdio server started");
