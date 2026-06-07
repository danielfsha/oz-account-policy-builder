/**
 * OZ Policy Builder — MCP Tools
 *
 * Exposes the record → synthesize → emit → harness pipeline to AI agents.
 * All tools are stateless and deterministic — no side effects, no auto-deploy.
 *
 * Tool inventory:
 *   1. record_transaction   — fetch tx from Horizon and build a CallManifest
 *   2. synthesize_policy    — run decision tree on a CallManifest → PolicySpec
 *   3. emit_policy_crate    — fill Rust templates → EmittedCrate (files as strings)
 *   4. run_harness          — permit + 5 deny-case tests
 *   5. list_primitives      — enumerate OZ policy primitives
 *   6. answer_clarification — resolve a pending clarification in a PolicySpec
 */

import { McpServer } from "@modelcontextprotocol/sdk/server/mcp.js";
import {
  RecordTransactionSchema,
  SynthesizePolicySchema,
  EmitPolicySchema,
  RunHarnessSchema,
  ListPrimitivesSchema,
  AnswerClarificationSchema,
  createSuccessResponse,
  createErrorResponse,
} from "../types";
import {
  fetchTransactionFromHorizon,
  buildCallManifest,
  synthesizePolicy,
  emitPolicyCrate,
  runHarness,
  OZ_PRIMITIVES,
  applyConstraintOverride,
} from "../pipeline";

export function registerPolicyTools(server: McpServer, _env: Env, _props: Record<string, never>) {
  // ── 1. record_transaction ──────────────────────────────────────────────────
  server.tool(
    "record_transaction",
    `Record a Stellar transaction and extract a structured CallManifest.
Provide a transaction hash and network, and the tool will:
- Fetch the transaction from Horizon
- Decode the XDR to extract contract calls, auth entries, and SAC transfer events
- Return a JSON CallManifest ready for synthesize_policy

Use this as the first step in the record → synthesize → emit workflow.`,
    RecordTransactionSchema,
    async (args) => {
      try {
        const { tx_hash, network, invoking_account } = args;
        const rawTx = await fetchTransactionFromHorizon(tx_hash, network as any);
        const manifest = buildCallManifest(rawTx, invoking_account);
        return createSuccessResponse(
          `Recorded transaction \`${tx_hash.slice(0, 12)}…\` on ${network}`,
          { manifest }
        );
      } catch (err: any) {
        return createErrorResponse("Failed to record transaction", { message: err.message });
      }
    }
  );

  // ── 2. synthesize_policy ───────────────────────────────────────────────────
  server.tool(
    "synthesize_policy",
    `Synthesize an OZ account policy from a recorded CallManifest.
Runs the decision tree to produce:
- A context rule (contracts, functions, lifetime)
- Up to 5 policy layers (spending_limit, time_window, simple_threshold, etc.)
- Whether existing OZ primitives suffice or net-new code is needed
- Clarification questions if parameters are ambiguous

If the spec has pending clarifications, use answer_clarification to resolve them before emitting.`,
    SynthesizePolicySchema,
    async (args) => {
      try {
        const manifest = JSON.parse(args.manifest_json);
        const constraints = {
          amount_cap: args.amount_cap ? BigInt(args.amount_cap) : undefined,
          time_window_seconds: args.time_window_seconds,
          lifetime_seconds: args.lifetime_seconds,
          allow_slippage_percent: args.max_slippage_percent,
        };
        const spec = synthesizePolicy(manifest, constraints);

        const hasClarifications = spec.clarifications_needed?.length > 0;
        const msg = hasClarifications
          ? `Policy synthesized — ${spec.clarifications_needed.length} clarification(s) needed before emitting`
          : `Policy synthesized — ready to emit (${spec.policies.length} layers, mode: ${spec.composition_mode})`;

        return createSuccessResponse(msg, {
          spec,
          clarifications: spec.clarifications_needed,
          summary: {
            policy_name: spec.policy_name,
            mode: spec.composition_mode,
            layers: spec.policies.map((p: any) => ({
              kind: p.kind,
              oz_primitive: p.oz_primitive,
              description: p.description,
            })),
            context_rule: spec.context_rule,
          },
        });
      } catch (err: any) {
        return createErrorResponse("Failed to synthesize policy", { message: err.message });
      }
    }
  );

  // ── 3. emit_policy_crate ───────────────────────────────────────────────────
  server.tool(
    "emit_policy_crate",
    `Generate a compilable Rust Soroban policy crate from a PolicySpec.
Returns all file contents as strings (Cargo.toml, src/lib.rs, REVIEW.md, .gitignore).

⚠️  AUTO-DEPLOY IS INTENTIONALLY NOT IMPLEMENTED.
The user must review the generated code, run the harness, and deploy manually.

The REVIEW.md contains a pre-deployment checklist — always show it to the user.`,
    EmitPolicySchema,
    async (args) => {
      try {
        const spec = JSON.parse(args.spec_json);

        if (spec.clarifications_needed?.length > 0) {
          return createErrorResponse(
            "Policy has unresolved clarifications — run answer_clarification first",
            { pending: spec.clarifications_needed.map((c: any) => c.question) }
          );
        }

        const emitted = emitPolicyCrate(spec, args.output_dir);

        const fileList = Object.keys(emitted.files).join(", ");
        return createSuccessResponse(
          `Generated policy crate \`${emitted.summary.crate_name}\` (${emitted.summary.layer_count} layers, ${emitted.summary.mode} mode)`,
          {
            crate_path: emitted.crate_path,
            files: emitted.files,
            review_md: emitted.review_md,
            summary: emitted.summary,
            next_steps: [
              `1. Review REVIEW.md checklist`,
              `2. Run: synthesize_policy → run_harness to verify permit/deny cases pass`,
              `3. Build: stellar contract build`,
              `4. Deploy manually: stellar contract deploy --wasm target/.../${emitted.summary.crate_name.replace(/-/g, '_')}.wasm`,
            ],
          }
        );
      } catch (err: any) {
        return createErrorResponse("Failed to emit policy crate", { message: err.message });
      }
    }
  );

  // ── 4. run_harness ─────────────────────────────────────────────────────────
  server.tool(
    "run_harness",
    `Run the permit/deny simulation harness against a synthesized policy.
Tests:
- PERMIT: the original recorded transaction must pass
- DENY (5 mutations): amount×2, wrong asset, wrong contract, extra function call, out-of-window

All 5 deny cases must fail for the harness to pass.
If any deny case passes (i.e., the policy is too permissive), re-synthesize with tighter constraints.`,
    RunHarnessSchema,
    async (args) => {
      try {
        const spec = JSON.parse(args.spec_json);
        const manifest = JSON.parse(args.manifest_json);
        const report = runHarness(spec, manifest);

        const status = report.passed ? "✅ ALL CASES PASSED" : "❌ HARNESS FAILED";
        return createSuccessResponse(`Harness complete — ${status}`, {
          passed: report.passed,
          permit: {
            passed: report.permit_result.passed,
            details: report.permit_result.details,
          },
          deny_cases: report.deny_results.map((r: any) => ({
            name: r.case_name,
            passed: r.passed,
            mutation: r.mutation,
            details: r.details,
          })),
          full_report: report.report,
          recommendation: report.passed
            ? "Safe to emit policy crate with emit_policy_crate"
            : "Re-run synthesize_policy with tighter constraints before emitting",
        });
      } catch (err: any) {
        return createErrorResponse("Harness failed to run", { message: err.message });
      }
    }
  );

  // ── 5. list_primitives ─────────────────────────────────────────────────────
  server.tool(
    "list_primitives",
    `List all available OpenZeppelin account policy primitives.
Use this to understand what's available before synthesizing a policy.
Each primitive can be composed without writing new Rust code.`,
    ListPrimitivesSchema,
    async () => {
      return createSuccessResponse("OpenZeppelin Account Policy Primitives", {
        primitives: OZ_PRIMITIVES,
        note: "Primitives are composed first. Net-new codegen only happens when no primitive can express the constraint.",
        oz_crate: "stellar-accounts = \"=0.7.1\"",
        docs: "https://docs.openzeppelin.com/stellar-contracts/accounts/policies",
      });
    }
  );

  // ── 6. answer_clarification ────────────────────────────────────────────────
  server.tool(
    "answer_clarification",
    `Answer a pending clarification question in a PolicySpec.
After synthesize_policy returns clarifications, use this tool to provide answers
and get an updated spec. Once all clarifications are resolved, call emit_policy_crate.

Example: if the synthesizer asks "should I cap at 50 USDC or 100 USDC per week?",
call this with field="amount_cap" and answer="100000000" (100 USDC in raw units).`,
    AnswerClarificationSchema,
    async (args) => {
      try {
        const spec = JSON.parse(args.spec_json);
        const updatedSpec = applyConstraintOverride(spec, args.field, args.answer);

        const remaining = updatedSpec.clarifications_needed?.length ?? 0;
        const msg = remaining === 0
          ? "All clarifications resolved — ready to emit"
          : `Clarification applied — ${remaining} question(s) remaining`;

        return createSuccessResponse(msg, {
          spec: updatedSpec,
          remaining_clarifications: updatedSpec.clarifications_needed,
        });
      } catch (err: any) {
        return createErrorResponse("Failed to apply clarification", { message: err.message });
      }
    }
  );
}
