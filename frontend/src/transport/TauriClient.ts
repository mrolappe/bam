// Tauri `invoke`/event transport (P9.3). This is the only file allowed to
// import `@tauri-apps/api` — components go through `BamClient`, never this
// class directly.
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
import type { BamClient } from "./BamClient";
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

type Invoke = <T>(cmd: string, args?: Record<string, unknown>) => Promise<T>;
type Listen = <T>(
  event: string,
  handler: (ev: { payload: T }) => void,
) => Promise<() => void>;

export interface TauriClientOptions {
  invoke?: Invoke;
  listen?: Listen;
}

export class TauriClient implements BamClient {
  private invoke: Invoke;
  private listen: Listen;

  constructor(opts: TauriClientOptions = {}) {
    this.invoke = opts.invoke ?? (tauriInvoke as Invoke);
    this.listen = opts.listen ?? (tauriListen as Listen);
  }

  searchPackages(req: SearchPackagesRequest) {
    return this.invoke<SearchPackagesResponse>("search_packages", { req });
  }
  searchWindow(req: SearchWindowRequest) {
    return this.invoke<SearchWindowResponse>("search_window", { req });
  }
  getPackage(req: GetPackageRequest) {
    return this.invoke<GetPackageResponse>("get_package", { req });
  }
  parseQuery(req: ParseQueryRequest) {
    return this.invoke<ParseQueryResponse>("parse_query", { req });
  }
  filterIds(req: FilterIdsRequest) {
    return this.invoke<FilterIdsResponse>("filter_ids", { req });
  }
  listCategories() {
    return this.invoke<ListCategoriesResponse>("list_categories");
  }
  selectByQuery(req: SelectByQueryRequest) {
    return this.invoke<SelectByQueryResponse>("select_by_query", { req });
  }
  async saveAs(req: SaveAsRequest) {
    await this.invoke<void>("save_as", { req });
  }
  async load(req: LoadRequest) {
    await this.invoke<void>("load", { req });
  }
  async deleteSelection(req: DeleteSelectionRequest) {
    await this.invoke<void>("delete_selection", { req });
  }
  listSelections() {
    return this.invoke<ListSelectionsResponse>("list_selections");
  }
  startIngest(req: StartIngestRequest) {
    return this.invoke<OperationId>("start_ingest", { req });
  }
  operationStatus(operation: OperationId) {
    return this.invoke<OperationStatusResponse>("operation_status", { operation });
  }

  async *progress(operation: OperationId, signal?: AbortSignal): AsyncIterable<ProgressEvent> {
    const queue: ProgressEvent[] = [];
    let resolveNext: (() => void) | null = null;
    let done = false;

    const unlisten = await this.listen<ProgressEvent>(`progress:${operation}`, (ev) => {
      queue.push(ev.payload);
      resolveNext?.();
    });
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
      unlisten();
    }
  }
}
