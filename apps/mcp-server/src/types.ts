import { z } from "zod";

// ── MCP response types ────────────────────────────────────────────────────────

export interface McpTextContent {
  type: "text";
  text: string;
  isError?: boolean;
  [key: string]: unknown;
}

export interface McpResponse {
  content: McpTextContent[];
  [key: string]: unknown;
}

export function createSuccessResponse(message: string, data?: any): McpResponse {
  let text = `✅ **${message}**`;
  if (data !== undefined) {
    text += `\n\n\`\`\`json\n${JSON.stringify(data, null, 2)}\n\`\`\``;
  }
  return { content: [{ type: "text", text }] };
}

export function createErrorResponse(message: string, details?: any): McpResponse {
  let text = `❌ **Error:** ${message}`;
  if (details !== undefined) {
    text += `\n\n\`\`\`json\n${JSON.stringify(details, null, 2)}\n\`\`\``;
  }
  return { content: [{ type: "text", text, isError: true }] };
}

// ── Tool input schemas ────────────────────────────────────────────────────────

export const RecordTransactionSchema = {
  tx_hash: z.string().min(1).describe(
    "Stellar transaction hash (64 hex chars). Or pass a JSON CallManifest directly as a string for testing."
  ),
  network: z.enum(["mainnet", "testnet", "futurenet"]).default("testnet").describe(
    "Stellar network"
  ),
  invoking_account: z.string().optional().describe(
    "Override the invoking account G-address (useful for simulations)"
  ),
};

export const SynthesizePolicySchema = {
  manifest_json: z.string().min(1).describe(
    "JSON-encoded CallManifest from record_transaction"
  ),
  amount_cap: z.string().optional().describe(
    "Override spending cap as raw u128 string (e.g. '1000000000' = 100 USDC at 7 decimals)"
  ),
  time_window_seconds: z.number().positive().optional().describe(
    "Override time window: 86400=daily, 604800=weekly, 2592000=monthly"
  ),
  lifetime_seconds: z.number().positive().optional().describe(
    "Context rule lifetime in seconds (default: 31536000 = 1 year)"
  ),
  max_slippage_percent: z.number().min(0).max(100).optional().describe(
    "For swap transactions: max allowed slippage % (e.g. 5.0)"
  ),
};

export const EmitPolicySchema = {
  spec_json: z.string().min(1).describe(
    "JSON-encoded PolicySpec from synthesize_policy"
  ),
  output_dir: z.string().optional().describe(
    "Output directory prefix (default: 'generated')"
  ),
};

export const RunHarnessSchema = {
  spec_json: z.string().min(1).describe(
    "JSON-encoded PolicySpec from synthesize_policy"
  ),
  manifest_json: z.string().min(1).describe(
    "JSON-encoded CallManifest from record_transaction"
  ),
};

export const ListPrimitivesSchema = {};

export const AnswerClarificationSchema = {
  spec_json: z.string().min(1).describe(
    "JSON-encoded PolicySpec with pending clarifications"
  ),
  field: z.string().min(1).describe(
    "The clarification field to answer (e.g. 'amount_cap', 'time_window_seconds')"
  ),
  answer: z.string().min(1).describe(
    "The answer or value (e.g. '100000000' for amount_cap)"
  ),
};
