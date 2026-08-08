// The one seam (P9.1): every component talks to this interface, never to
// `@tauri-apps/api` or `fetch` directly, so a component can never quietly
// become Tauri-only or web-only.
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

export interface BamClient {
  searchPackages(req: SearchPackagesRequest): Promise<SearchPackagesResponse>;
  searchWindow(req: SearchWindowRequest): Promise<SearchWindowResponse>;
  getPackage(req: GetPackageRequest): Promise<GetPackageResponse>;
  /** Rejects with {@link BamApiError} (span set when the language can pin one) on a bad query. */
  parseQuery(req: ParseQueryRequest): Promise<ParseQueryResponse>;
  filterIds(req: FilterIdsRequest): Promise<FilterIdsResponse>;
  listCategories(): Promise<ListCategoriesResponse>;
  selectByQuery(req: SelectByQueryRequest): Promise<SelectByQueryResponse>;
  saveAs(req: SaveAsRequest): Promise<void>;
  load(req: LoadRequest): Promise<void>;
  deleteSelection(req: DeleteSelectionRequest): Promise<void>;
  listSelections(): Promise<ListSelectionsResponse>;
  startIngest(req: StartIngestRequest): Promise<OperationId>;
  operationStatus(operation: OperationId): Promise<OperationStatusResponse>;
  /** Flips a package's `marked` selection membership (I7), returning the new state. */
  toggle(packageId: number): Promise<boolean>;
  /** Terminates on `Finished` or when `signal` aborts — never hangs open. */
  progress(operation: OperationId, signal?: AbortSignal): AsyncIterable<ProgressEvent>;
}

/**
 * Uniform shape both transports throw on a failed call. `span` (byte offsets
 * `[start, end)` into the source) is only ever set for a query-parse
 * failure, mirroring `bam_core::query::lang::ParseError` (P3.5's reference).
 */
export class BamApiError extends Error {
  span?: [number, number];

  constructor(message: string, span?: [number, number] | null) {
    super(message);
    this.name = "BamApiError";
    this.span = span ?? undefined;
  }
}
