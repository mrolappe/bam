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
  /** Terminates on `Finished` or when `signal` aborts — never hangs open. */
  progress(operation: OperationId, signal?: AbortSignal): AsyncIterable<ProgressEvent>;
}
