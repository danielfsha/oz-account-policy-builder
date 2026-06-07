/**
 * Fetch a Stellar transaction from Horizon and return raw decoded data.
 */

export type Network = "mainnet" | "testnet" | "futurenet";

const HORIZON_URLS: Record<Network, string> = {
  mainnet: "https://horizon.stellar.org",
  testnet: "https://horizon-testnet.stellar.org",
  futurenet: "https://horizon-futurenet.stellar.org",
};

export interface RawTransaction {
  hash: string;
  network: Network;
  ledger: number;
  created_at: string;
  source_account: string;
  successful: boolean;
  envelope_xdr: string;
  result_meta_xdr: string;
  /** Parsed operations from the envelope */
  operations?: any[];
}

/** Fetch a transaction by hash from Horizon. */
export async function fetchTransactionFromHorizon(
  txHash: string,
  network: Network = "testnet"
): Promise<RawTransaction> {
  const baseUrl = HORIZON_URLS[network];
  const url = `${baseUrl}/transactions/${txHash}`;

  const res = await fetch(url, {
    headers: { Accept: "application/json" },
  });

  if (!res.ok) {
    const body = await res.text();
    throw new Error(`Horizon returned ${res.status} for tx ${txHash}: ${body.slice(0, 200)}`);
  }

  const data: any = await res.json();

  if (!data.successful) {
    throw new Error(`Transaction ${txHash} failed on-chain — cannot record a failed transaction`);
  }

  return {
    hash: data.hash,
    network,
    ledger: data.ledger,
    created_at: data.created_at,
    source_account: data.source_account,
    successful: data.successful,
    envelope_xdr: data.envelope_xdr,
    result_meta_xdr: data.result_meta_xdr,
  };
}
