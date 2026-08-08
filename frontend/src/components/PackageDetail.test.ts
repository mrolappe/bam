import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import PackageDetail from "./PackageDetail.vue";
import { bamClientKey } from "../composables/useBamClient";
import { mockClient } from "../test-utils/mockClient";

describe("PackageDetail", () => {
  it("renders nothing when no package is selected", () => {
    const wrapper = mount(PackageDetail, {
      props: { packageId: null },
      global: { provide: { [bamClientKey as symbol]: mockClient() } },
    });
    expect(wrapper.text()).toBe("");
  });

  it("fetches and renders the selected package's fields", async () => {
    const getPackage = vi.fn(async () => ({
      package: {
        id: 7,
        dir: "games/action",
        file: "shoot.lha",
        name: "shoot",
        version: "1.2",
        date_precision: "exact",
        landing_id: 1,
        description: "a shooter",
      },
    }));
    const wrapper = mount(PackageDetail, {
      props: { packageId: 7 },
      global: { provide: { [bamClientKey as symbol]: mockClient({ getPackage }) } },
    });
    await flushPromises();

    expect(getPackage).toHaveBeenCalledWith({ id: 7 });
    expect(wrapper.text()).toContain("shoot");
    expect(wrapper.text()).toContain("games/action");
    expect(wrapper.text()).toContain("a shooter");
  });

  it("re-fetches when packageId changes", async () => {
    const getPackage = vi.fn(async (req: { id: number }) => ({
      package: {
        id: req.id,
        dir: "d",
        file: "f",
        name: `pkg${req.id}`,
        date_precision: "exact",
        landing_id: 1,
      },
    }));
    const wrapper = mount(PackageDetail, {
      props: { packageId: 1 },
      global: { provide: { [bamClientKey as symbol]: mockClient({ getPackage }) } },
    });
    await flushPromises();
    expect(wrapper.text()).toContain("pkg1");

    await wrapper.setProps({ packageId: 2 });
    await flushPromises();
    expect(wrapper.text()).toContain("pkg2");
  });
});
