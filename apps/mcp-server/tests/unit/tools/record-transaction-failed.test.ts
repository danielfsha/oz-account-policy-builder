import { describe, it, expect, vi } from "vitest";
import { fetchTransactionFromHorizon } from "../../../src/pipeline/recorder";

const TX_HASH = "1ebfec4d31ba270fa6cb5d6b0c3ee4bde1ebb0f24da8fb47c87cdc1606d9d252";

describe("fetchTransactionFromHorizon", () => {
  it("throws a descriptive error when the transaction failed on-chain", async () => {
    // Simulate Horizon returning a failed transaction (successful: false)
    // Per Stellar docs: the `successful` boolean field indicates if the tx was applied
    (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      ok: true,
      json: async () => ({
        hash: TX_HASH,
        ledger: 123456,
        created_at: "2025-01-01T00:00:00Z",
        source_account: "GCO2IP3MJNUOKS4PUDI4C7LGGMQDJGXG3COYX3WSB4HHNAHKYV5YL3VC",
        successful: false,
        envelope_xdr: "AAAA...",
        result_xdr: "AAAAAAAAAGT////7AAAAAA==",
        result_meta_xdr: "BBBB...",
        fee_charged: 100,
        max_fee: 100,
        operation_count: 1,
      }),
    });

    await expect(
      fetchTransactionFromHorizon(TX_HASH, "mainnet")
    ).rejects.toThrow(
      /failed on-chain.*mainnet.*cannot extract a CallManifest/
    );
  });

  it("includes network name in the error for failed transactions", async () => {
    (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      ok: true,
      json: async () => ({
        hash: TX_HASH,
        ledger: 999,
        created_at: "2025-06-01T00:00:00Z",
        source_account: "GABCDEF...",
        successful: false,
        envelope_xdr: "AAAA...",
        result_meta_xdr: "BBBB...",
      }),
    });

    await expect(
      fetchTransactionFromHorizon(TX_HASH, "testnet")
    ).rejects.toThrow(/network: testnet/);
  });

  it("throws when Horizon returns 404 (tx not found on network)", async () => {
    (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      ok: false,
      status: 404,
      text: async () => JSON.stringify({
        type: "https://stellar.org/horizon-errors/not_found",
        title: "Resource Missing",
        status: 404,
        detail: "The resource at the url requested was not found.",
      }),
    });

    await expect(
      fetchTransactionFromHorizon(TX_HASH, "mainnet")
    ).rejects.toThrow(/Horizon returned 404/);
  });

  it("returns RawTransaction for a successful transaction", async () => {
    // Simulate a successful Horizon response matching the documented format
    (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      ok: true,
      json: async () => ({
        hash: TX_HASH,
        ledger: 27956256,
        created_at: "2025-01-27T22:13:17Z",
        source_account: "GCO2IP3MJNUOKS4PUDI4C7LGGMQDJGXG3COYX3WSB4HHNAHKYV5YL3VC",
        successful: true,
        envelope_xdr: "AAAAAJ2kP2xLaOVLj6DRwX1mMyA0mubYnYvu...",
        result_xdr: "AAAAAAAAAGQAAAAAAAAAAQAAAAAAAAABAAAAAAAAAAA=",
        result_meta_xdr: "AAAAAQAAAAIAAAADAaqUIAAAAAAAAAAA...",
        fee_charged: 100,
        max_fee: 100,
        operation_count: 1,
        memo: "298424",
        memo_type: "text",
      }),
    });

    const result = await fetchTransactionFromHorizon(TX_HASH, "testnet");
    expect(result.successful).toBe(true);
    expect(result.hash).toBe(TX_HASH);
    expect(result.network).toBe("testnet");
    expect(result.ledger).toBe(27956256);
    expect(result.source_account).toBe("GCO2IP3MJNUOKS4PUDI4C7LGGMQDJGXG3COYX3WSB4HHNAHKYV5YL3VC");
    expect(result.envelope_xdr).toBeDefined();
    expect(result.result_meta_xdr).toBeDefined();
  });

  it("calls the correct Horizon URL based on network", async () => {
    (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      ok: true,
      json: async () => ({
        hash: TX_HASH,
        ledger: 100,
        created_at: "2025-01-01T00:00:00Z",
        source_account: "GABCDEF...",
        successful: true,
        envelope_xdr: "AAAA...",
        result_meta_xdr: "BBBB...",
      }),
    });

    await fetchTransactionFromHorizon(TX_HASH, "mainnet");

    expect(global.fetch).toHaveBeenCalledWith(
      `https://horizon.stellar.org/transactions/${TX_HASH}`,
      { headers: { Accept: "application/json" } }
    );
  });

  it("calls testnet Horizon URL for testnet network", async () => {
    (global.fetch as ReturnType<typeof vi.fn>).mockResolvedValueOnce({
      ok: true,
      json: async () => ({
        hash: TX_HASH,
        ledger: 100,
        created_at: "2025-01-01T00:00:00Z",
        source_account: "GABCDEF...",
        successful: true,
        envelope_xdr: "AAAA...",
        result_meta_xdr: "BBBB...",
      }),
    });

    await fetchTransactionFromHorizon(TX_HASH, "testnet");

    expect(global.fetch).toHaveBeenCalledWith(
      `https://horizon-testnet.stellar.org/transactions/${TX_HASH}`,
      { headers: { Accept: "application/json" } }
    );
  });
});
