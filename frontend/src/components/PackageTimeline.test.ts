import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import PackageTimeline from "./PackageTimeline.vue";
import { bamClientKey } from "../composables/useBamClient";
import { mockClient } from "../test-utils/mockClient";
import type { Package } from "../generated/types";

function pkg(id: number, uploaded_on: string, date_precision: string): Package {
  return {
    id,
    dir: "d",
    file: `f${id}`,
    name: `pkg${id}`,
    date_precision,
    uploaded_on,
    landing_id: 1,
  };
}

describe("PackageTimeline", () => {
  it("buckets uploads by year across a multi-year fixture", async () => {
    const packages = [
      pkg(1, "2023-03-01", "exact"),
      pkg(2, "2023-11-01", "exact"),
      pkg(3, "2024-01-01", "exact"),
      pkg(4, "2026-06-01", "exact"),
    ];
    const wrapper = mount(PackageTimeline, {
      global: {
        provide: {
          [bamClientKey as symbol]: mockClient({
            searchPackages: async () => ({ packages }),
          }),
        },
      },
    });
    await flushPromises();

    expect(wrapper.get('[data-testid="year-2023"]').text()).toContain("2");
    expect(wrapper.get('[data-testid="year-2024"]').text()).toContain("1");
    expect(wrapper.get('[data-testid="year-2026"]').text()).toContain("1");
    expect(wrapper.find('[data-testid="year-2025"]').exists()).toBe(false);
  });

  it("renders week-precision bars distinguishably from exact ones", async () => {
    const packages = [pkg(1, "2026-01-01", "exact"), pkg(2, "2026-01-01", "week")];
    const wrapper = mount(PackageTimeline, {
      global: {
        provide: {
          [bamClientKey as symbol]: mockClient({
            searchPackages: async () => ({ packages }),
          }),
        },
      },
    });
    await flushPromises();

    const exactBar = wrapper.get('[data-testid="bar-exact-2026"]');
    const weekBar = wrapper.get('[data-testid="bar-week-2026"]');
    expect(exactBar.classes()).toContain("precision-exact");
    expect(weekBar.classes()).toContain("precision-week");
    expect(exactBar.classes()).not.toEqual(weekBar.classes());
  });

  it("reflects the active predicate rather than the whole archive", async () => {
    const searchPackages = vi.fn(async () => ({ packages: [] }));
    const wrapper = mount(PackageTimeline, {
      props: { predicate: { FullText: "demo" } },
      global: {
        provide: {
          [bamClientKey as symbol]: mockClient({ searchPackages }),
        },
      },
    });
    await flushPromises();

    expect(searchPackages).toHaveBeenCalledWith({ predicate: { FullText: "demo" } });

    await wrapper.setProps({ predicate: { FullText: "other" } });
    await flushPromises();

    expect(searchPackages).toHaveBeenCalledWith({ predicate: { FullText: "other" } });
  });
});
