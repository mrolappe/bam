import { inject, type InjectionKey } from "vue";
import type { BamClient } from "../transport/BamClient";

export const bamClientKey: InjectionKey<BamClient> = Symbol("BamClient");

export function useBamClient(): BamClient {
  const client = inject(bamClientKey);
  if (!client) {
    throw new Error("useBamClient() called without a provided BamClient");
  }
  return client;
}
