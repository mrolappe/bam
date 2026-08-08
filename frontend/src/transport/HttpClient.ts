// `fetch` + SSE transport for the browser/`bam-server` host (P9.2). This is
// the only file allowed to call `fetch` for API traffic — components go
// through `BamClient`, never this class directly.
import { BamApiError, type BamClient } from "./BamClient";
import type {
  SearchPackagesRequest,
  SearchPackagesResponse,
  SearchWindowRequest,
  SearchWindowResponse,
  GetPackageRequest,
  GetPackageResponse,
  ParseQueryRequest,
  ParseQueryResponse,
  FilterIdsRequest,
  FilterIdsResponse,
  ListCategoriesResponse,
  SelectByQueryRequest,
  SelectByQueryResponse,
  SaveAsRequest,
  LoadRequest,
  DeleteSelectionRequest,
  ListSelectionsResponse,
  StartIngestRequest,
  OperationStatusResponse,
  OperationId,
  ProgressEvent,
} from "../generated/types";

/** Minimal shape of the DOM `EventSource`, narrowed for injection in tests. */
export interface EventSourceLike {
  onmessage: ((ev: { data: string }) => void) | null;
  onerror: ((ev: unknown) => void) | null;
  close(): void;
}

export interface HttpClientOptions {
  baseUrl?: string;
  fetchImpl?: typeof fetch;
  eventSourceFactory?: (url: string) => EventSourceLike;
}

export class HttpClient implements BamClient {
  private baseUrl: string;
  private fetchImpl: typeof fetch;
  private eventSourceFactory: (url: string) => EventSourceLike;

  constructor(opts: HttpClientOptions = {}) {
    this.baseUrl = opts.baseUrl ?? "";
    this.fetchImpl = opts.fetchImpl ?? fetch;
    this.eventSourceFactory =
      opts.eventSourceFactory ?? ((url) => new EventSource(url) as unknown as EventSourceLike);
  }

  private async post<Req, Res>(path: string, req: Req): Promise<Res> {
    const res = await this.fetchImpl(`${this.baseUrl}${path}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(req),
    });
    if (!res.ok) {
      const body = (await res.json().catch(() => null)) as {
        error?: string;
        span?: [number, number] | null;
      } | null;
      throw new BamApiError(body?.error ?? `${path} failed: ${res.status}`, body?.span);
    }
    return (await res.json()) as Res;
  }

  searchPackages(req: SearchPackagesRequest) {
    return this.post<SearchPackagesRequest, SearchPackagesResponse>("/api/search-packages", req);
  }
  searchWindow(req: SearchWindowRequest) {
    return this.post<SearchWindowRequest, SearchWindowResponse>("/api/search-window", req);
  }
  getPackage(req: GetPackageRequest) {
    return this.post<GetPackageRequest, GetPackageResponse>("/api/get-package", req);
  }
  parseQuery(req: ParseQueryRequest) {
    return this.post<ParseQueryRequest, ParseQueryResponse>("/api/parse-query", req);
  }
  filterIds(req: FilterIdsRequest) {
    return this.post<FilterIdsRequest, FilterIdsResponse>("/api/filter-ids", req);
  }
  listCategories() {
    return this.post<Record<string, never>, ListCategoriesResponse>("/api/list-categories", {});
  }
  selectByQuery(req: SelectByQueryRequest) {
    return this.post<SelectByQueryRequest, SelectByQueryResponse>("/api/select-by-query", req);
  }
  async saveAs(req: SaveAsRequest) {
    await this.post<SaveAsRequest, unknown>("/api/save-as", req);
  }
  async load(req: LoadRequest) {
    await this.post<LoadRequest, unknown>("/api/load", req);
  }
  async deleteSelection(req: DeleteSelectionRequest) {
    await this.post<DeleteSelectionRequest, unknown>("/api/delete-selection", req);
  }
  listSelections() {
    return this.post<Record<string, never>, ListSelectionsResponse>("/api/list-selections", {});
  }
  async startIngest(req: StartIngestRequest) {
    const res = await this.post<StartIngestRequest, { operation: OperationId }>(
      "/api/start-ingest",
      req,
    );
    return res.operation;
  }
  operationStatus(operation: OperationId) {
    return this.post<{ operation: OperationId }, OperationStatusResponse>(
      "/api/operation-status",
      { operation },
    );
  }
  async toggle(packageId: number) {
    const res = await this.post<{ package_id: number }, { marked: boolean }>("/api/toggle", {
      package_id: packageId,
    });
    return res.marked;
  }

  async *progress(operation: OperationId, signal?: AbortSignal): AsyncIterable<ProgressEvent> {
    const source = this.eventSourceFactory(`${this.baseUrl}/api/progress/${operation}`);
    const queue: ProgressEvent[] = [];
    let resolveNext: (() => void) | null = null;
    let done = false;

    source.onmessage = (ev) => {
      queue.push(JSON.parse(ev.data) as ProgressEvent);
      resolveNext?.();
    };
    source.onerror = () => {
      done = true;
      resolveNext?.();
    };
    const onAbort = () => {
      done = true;
      resolveNext?.();
    };
    signal?.addEventListener("abort", onAbort);

    try {
      while (!done) {
        if (queue.length === 0) {
          await new Promise<void>((resolve) => {
            resolveNext = resolve;
          });
          resolveNext = null;
          continue;
        }
        const event = queue.shift()!;
        yield event;
        if ("Finished" in event) {
          done = true;
        }
      }
    } finally {
      signal?.removeEventListener("abort", onAbort);
      source.close();
    }
  }
}
