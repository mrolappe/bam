// One shared suite run against both transports (P9.1's second test): if a
// method only worked on one of them, this is where it would show up.
import { describe, expect, it, vi } from "vitest";
import type { BamClient } from "./BamClient";
import { HttpClient, type EventSourceLike } from "./HttpClient";
import { TauriClient } from "./TauriClient";
import type { ProgressEvent } from "../generated/types";

interface Harness {
  client: BamClient;
  emit(event: ProgressEvent): void;
  end(): void;
}

function httpHarness(): Harness {
  let onmessage: ((ev: { data: string }) => void) | null = null;
  const fakeSource: EventSourceLike = {
    onmessage: null,
    onerror: null,
    close: vi.fn(),
  };
  const fetchImpl = vi.fn(async (url: string) => {
    const body = url.endsWith("/api/list-categories")
      ? { categories: [] }
      : url.endsWith("/api/list-selections")
        ? { selections: [] }
        : url.endsWith("/api/start-ingest")
          ? { operation: 1 }
          : {};
    return new Response(JSON.stringify(body), { status: 200 });
  }) as unknown as typeof fetch;

  const client = new HttpClient({
    fetchImpl,
    eventSourceFactory: () => {
      onmessage = null;
      Object.defineProperty(fakeSource, "onmessage", {
        get: () => onmessage,
        set: (v) => (onmessage = v),
        configurable: true,
      });
      return fakeSource;
    },
  });

  return {
    client,
    emit: (event) => onmessage?.({ data: JSON.stringify(event) }),
    end: () => fakeSource.onerror?.(undefined),
  };
}

function tauriHarness(): Harness {
  let handler: ((ev: { payload: ProgressEvent }) => void) | null = null;
  const invoke = vi.fn(async (cmd: string) => {
    if (cmd === "list_categories") return { categories: [] };
    if (cmd === "list_selections") return { selections: [] };
    if (cmd === "start_ingest") return 1;
    return {};
  });
  const listen = vi.fn(async (_event: string, h: (ev: { payload: ProgressEvent }) => void) => {
    handler = h;
    return () => {
      handler = null;
    };
  });

  const client = new TauriClient({ invoke: invoke as never, listen: listen as never });

  return {
    client,
    emit: (event) => handler?.({ payload: event }),
    end: () => {
      /* Tauri transport has no separate end-of-stream signal; abort covers it. */
    },
  };
}

const transports: Array<[string, () => Harness]> = [
  ["HttpClient", httpHarness],
  ["TauriClient", tauriHarness],
];

describe.each(transports)("%s satisfies the BamClient contract", (_name, makeHarness) => {
  it("resolves listCategories and listSelections", async () => {
    const { client } = makeHarness();
    await expect(client.listCategories()).resolves.toEqual({ categories: [] });
    await expect(client.listSelections()).resolves.toEqual({ selections: [] });
  });

  it("startIngest resolves to an OperationId", async () => {
    const { client } = makeHarness();
    await expect(client.startIngest({ mode: "Fetch" })).resolves.toBe(1);
  });

  it("progress() yields events in order and terminates cleanly on Finished", async () => {
    const { client, emit } = makeHarness();
    const events: ProgressEvent[] = [
      { Started: { operation: 1, total: 2 } },
      { Advanced: { operation: 1, done: 1 } },
      { Finished: { operation: 1, outcome: "Success" } },
    ];

    const received: ProgressEvent[] = [];
    const iterationDone = (async () => {
      for await (const event of client.progress(1)) {
        received.push(event);
      }
    })();

    for (const event of events) {
      await Promise.resolve();
      emit(event);
    }

    await iterationDone;
    expect(received).toEqual(events);
  });

  it("progress() terminates cleanly when the AbortSignal fires", async () => {
    const { client, emit } = makeHarness();
    const controller = new AbortController();
    const received: ProgressEvent[] = [];

    const iterationDone = (async () => {
      for await (const event of client.progress(1, controller.signal)) {
        received.push(event);
      }
    })();

    await Promise.resolve();
    emit({ Started: { operation: 1, total: null } });
    await Promise.resolve();
    controller.abort();

    await iterationDone;
    expect(received).toEqual([{ Started: { operation: 1, total: null } }]);
  });
});
