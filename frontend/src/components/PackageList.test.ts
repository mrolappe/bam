// P9.1's first required test: renders against a mock BamClient, with
// neither TauriClient nor HttpClient present.
import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import PackageList from "./PackageList.vue";
import { bamClientKey } from "../composables/useBamClient";
import type { BamClient } from "../transport/BamClient";

function mockClient(overrides: Partial<BamClient> = {}): BamClient {
  return {
    searchPackages: vi.fn(async () => ({
      packages: [
        { id: 1, dir: "d", file: "f", name: "workbench", date_precision: "exact", landing_id: 1 },
      ],
    })),
    searchWindow: vi.fn(),
    getPackage: vi.fn(),
    parseQuery: vi.fn(),
    filterIds: vi.fn(),
    listCategories: vi.fn(),
    selectByQuery: vi.fn(),
    saveAs: vi.fn(),
    load: vi.fn(),
    deleteSelection: vi.fn(),
    listSelections: vi.fn(),
    startIngest: vi.fn(),
    operationStatus: vi.fn(),
    progress: vi.fn(),
    ...overrides,
  } as BamClient;
}

describe("PackageList", () => {
  it("renders packages returned by the injected mock client", async () => {
    const wrapper = mount(PackageList, {
      global: { provide: { [bamClientKey as symbol]: mockClient() } },
    });
    await flushPromises();
    expect(wrapper.text()).toContain("workbench");
  });
});
