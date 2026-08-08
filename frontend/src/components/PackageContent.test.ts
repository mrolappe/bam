import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import PackageContent from "./PackageContent.vue";
import { bamClientKey } from "../composables/useBamClient";
import { mockClient } from "../test-utils/mockClient";

describe("PackageContent", () => {
  it("renders file types and directory structure from an inventory fixture", async () => {
    const getInventory = vi.fn(async () => ({
      inventory: {
        files: [
          { path: "docs/readme.txt", size: 100, kind: "text" },
          { path: "pics/icon.iff", size: 200, kind: "image" },
          { path: "pics/screen.iff", size: 300, kind: "image" },
        ],
      },
    }));
    const wrapper = mount(PackageContent, {
      props: { packageId: 7 },
      global: { provide: { [bamClientKey as symbol]: mockClient({ getInventory }) } },
    });
    await flushPromises();

    expect(getInventory).toHaveBeenCalledWith({ package_id: 7 });
    expect(wrapper.find('[data-testid="kind-text"]').text()).toContain("1 files, 100 bytes");
    expect(wrapper.find('[data-testid="kind-image"]').text()).toContain("2 files, 500 bytes");
    expect(wrapper.find('[data-testid="dir-pics"]').exists()).toBe(true);
    expect(wrapper.find('[data-testid="not-analyzed"]').exists()).toBe(false);
  });

  it("shows a not-analyzed state when no inventory exists yet", async () => {
    const getInventory = vi.fn(async () => ({ inventory: null }));
    const wrapper = mount(PackageContent, {
      props: { packageId: 7 },
      global: { provide: { [bamClientKey as symbol]: mockClient({ getInventory }) } },
    });
    await flushPromises();

    expect(wrapper.find('[data-testid="not-analyzed"]').exists()).toBe(true);
  });
});
