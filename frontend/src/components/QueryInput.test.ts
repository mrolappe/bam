import { describe, expect, it, vi } from "vitest";
import { flushPromises, mount } from "@vue/test-utils";
import QueryInput from "./QueryInput.vue";
import { bamClientKey } from "../composables/useBamClient";
import { mockClient } from "../test-utils/mockClient";
import { BamApiError } from "../transport/BamClient";

describe("QueryInput", () => {
  it("debounces keystrokes into a single parseQuery call", async () => {
    vi.useFakeTimers();
    const parseQuery = vi.fn(async () => ({ predicate: { FullText: "x" } }));
    const wrapper = mount(QueryInput, {
      global: { provide: { [bamClientKey as symbol]: mockClient({ parseQuery }) } },
    });

    const input = wrapper.get("input");
    for (const partial of ["d", "di", "dir"]) {
      await input.setValue(partial);
      await vi.advanceTimersByTimeAsync(50);
    }
    expect(parseQuery).not.toHaveBeenCalled();

    await vi.advanceTimersByTimeAsync(150);
    expect(parseQuery).toHaveBeenCalledTimes(1);
    expect(parseQuery).toHaveBeenCalledWith({ src: "dir" });
    vi.useRealTimers();
  });

  it("emits the parsed predicate on a valid query", async () => {
    vi.useFakeTimers();
    const predicate = { FullText: "workbench" };
    const wrapper = mount(QueryInput, {
      global: {
        provide: {
          [bamClientKey as symbol]: mockClient({ parseQuery: vi.fn(async () => ({ predicate })) }),
        },
      },
    });

    await wrapper.get("input").setValue("workbench");
    await vi.advanceTimersByTimeAsync(200);
    vi.useRealTimers();
    await flushPromises();

    expect(wrapper.emitted("predicate")).toEqual([[predicate]]);
    expect(wrapper.find("[data-testid='query-error']").exists()).toBe(false);
  });

  it("renders the error span highlighted and keeps the previous predicate", async () => {
    vi.useFakeTimers();
    const parseQuery = vi
      .fn()
      .mockRejectedValue(new BamApiError("expected a value after '>'", [15, 16]));
    const wrapper = mount(QueryInput, {
      global: { provide: { [bamClientKey as symbol]: mockClient({ parseQuery }) } },
    });

    await wrapper.get("input").setValue("dir:util/* size>");
    await vi.advanceTimersByTimeAsync(200);
    vi.useRealTimers();
    await flushPromises();

    expect(wrapper.emitted("predicate")).toBeUndefined();
    const error = wrapper.get("[data-testid='query-error']");
    expect(error.text()).toContain("expected a value after '>'");
    // Byte offset [15, 16) is the trailing '>' — the offending span.
    const span = wrapper.get("[data-testid='error-span']");
    expect(span.text()).toBe(">");
  });
});
