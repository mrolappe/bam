// Tauri `invoke`/event transport (P9.3). This is the only file allowed to
// import `@tauri-apps/api` — components go through `BamClient`, never this
// class directly.
import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { listen as tauriListen } from "@tauri-apps/api/event";
import { BamApiError, type BamClient } from "./BamClient";
import type {
  SearchPackagesRequest,
  SearchPackagesResponse,
  SearchWindowRequest,
  SearchWindowResponse,
  GetPackageRequest,
  GetPackageResponse,
  GetInventoryRequest,
  GetInventoryResponse,
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

  /** Runs a Tauri command, rethrowing its `CmdError` payload as a {@link BamApiError}. */
  private async call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
    try {
      return await this.invoke<T>(cmd, args);
    } catch (e) {
      const err = e as { message?: string; span?: [number, number] | null };
      if (err && typeof err.message === "string") {
        throw new BamApiError(err.message, err.span);
      }
      throw e;
    }
  }

  searchPackages(req: SearchPackagesRequest) {
    return this.call<SearchPackagesResponse>("search_packages", { req });
  }
  searchWindow(req: SearchWindowRequest) {
    return this.call<SearchWindowResponse>("search_window", { req });
  }
  getPackage(req: GetPackageRequest) {
    return this.call<GetPackageResponse>("get_package", { req });
  }
  getInventory(req: GetInventoryRequest) {
    return this.call<GetInventoryResponse>("get_inventory", { req });
  }
  parseQuery(req: ParseQueryRequest) {
    return this.call<ParseQueryResponse>("parse_query", { req });
  }
  filterIds(req: FilterIdsRequest) {
    return this.call<FilterIdsResponse>("filter_ids", { req });
  }
  listCategories() {
    return this.call<ListCategoriesResponse>("list_categories");
  }
  selectByQuery(req: SelectByQueryRequest) {
    return this.call<SelectByQueryResponse>("select_by_query", { req });
  }
  async saveAs(req: SaveAsRequest) {
    await this.call<void>("save_as", { req });
  }
  async load(req: LoadRequest) {
    await this.call<void>("load", { req });
  }
  async deleteSelection(req: DeleteSelectionRequest) {
    await this.call<void>("delete_selection", { req });
  }
  listSelections() {
    return this.call<ListSelectionsResponse>("list_selections");
  }
  startIngest(req: StartIngestRequest) {
    return this.call<OperationId>("start_ingest", { req });
  }
  operationStatus(operation: OperationId) {
    return this.call<OperationStatusResponse>("operation_status", { operation });
  }
  async toggle(packageId: number) {
    const res = await this.call<{ marked: boolean }>("toggle", {
      req: { package_id: packageId },
    });
    return res.marked;
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
