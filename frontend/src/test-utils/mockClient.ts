// Shared `BamClient` stub for component tests (P9.4): every field the
// interface requires, so a component test only overrides what it exercises.
import { vi } from "vitest";
import type { BamClient } from "../transport/BamClient";

export function mockClient(overrides: Partial<BamClient> = {}): BamClient {
  return {
    searchPackages: vi.fn(async () => ({ packages: [] })),
    searchWindow: vi.fn(async () => ({ packages: [], total: 0 })),
    getPackage: vi.fn(async () => ({ package: null })),
    getInventory: vi.fn(async () => ({ inventory: null })),
    parseQuery: vi.fn(async () => ({ predicate: { FullText: "" } })),
    filterIds: vi.fn(),
    listCategories: vi.fn(),
    selectByQuery: vi.fn(),
    saveAs: vi.fn(),
    load: vi.fn(),
    deleteSelection: vi.fn(),
    listSelections: vi.fn(),
    startIngest: vi.fn(),
    operationStatus: vi.fn(),
    toggle: vi.fn(async () => true),
    progress: vi.fn(),
    ...overrides,
  } as BamClient;
}
