// P9.1's first required test (renders against a mock BamClient) plus P9.4's
// virtualization and selection-toggle tests.
import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import PackageList from "./PackageList.vue";
import { bamClientKey } from "../composables/useBamClient";
import { mockClient } from "../test-utils/mockClient";
import type { Package } from "../generated/types";

function fakePackage(id: number): Package {
  return { id, dir: "d", file: `f${id}`, name: `pkg${id}`, date_precision: "exact", landing_id: 1 };
}

describe("PackageList", () => {
  it("renders packages returned by the injected mock client", async () => {
    const wrapper = mount(PackageList, {
      global: {
        provide: {
          [bamClientKey as symbol]: mockClient({
            searchPackages: vi.fn(async () => ({ packages: [fakePackage(1)] })),
          }),
        },
      },
    });
    await flushPromises();
    expect(wrapper.text()).toContain("pkg1");
  });

  it("mounts a bounded number of row components for an 84,000-row result", async () => {
    const packages = Array.from({ length: 84_000 }, (_, i) => fakePackage(i));
    const wrapper = mount(PackageList, {
      global: {
        provide: {
          [bamClientKey as symbol]: mockClient({
            searchPackages: vi.fn(async () => ({ packages })),
          }),
        },
      },
    });
    await flushPromises();
    expect(wrapper.findAll("li").length).toBeLessThan(100);
  });

  it("toggles a package's mark through the client and reflects the new state", async () => {
    const toggle = vi.fn(async () => true);
    const wrapper = mount(PackageList, {
      global: {
        provide: {
          [bamClientKey as symbol]: mockClient({
            searchPackages: vi.fn(async () => ({ packages: [fakePackage(1)] })),
            toggle,
          }),
        },
      },
    });
    await flushPromises();

    await wrapper.get("[data-testid='mark-1']").trigger("click");
    await flushPromises();

    expect(toggle).toHaveBeenCalledWith(1);
    expect(wrapper.get("[data-testid='mark-1']").text()).toBe("✓");
  });

  it("emits select when a row is clicked", async () => {
    const wrapper = mount(PackageList, {
      global: {
        provide: {
          [bamClientKey as symbol]: mockClient({
            searchPackages: vi.fn(async () => ({ packages: [fakePackage(1)] })),
          }),
        },
      },
    });
    await flushPromises();

    await wrapper.get("li").trigger("click");
    expect(wrapper.emitted("select")).toEqual([[1]]);
  });
});
