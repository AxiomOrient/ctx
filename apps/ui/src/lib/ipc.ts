import { safeInvoke, isTauriEnvironment } from './tauri-utils';
import type { WorkspaceSummary, IndexSummary, DocMeta, DocReadResult, SaveResult, ComposeRequest, PromptBundle } from './types';

type UiError = { code: string; message: string; data?: any };

function normalizeError(e: any): UiError {
  if (e && typeof e === 'object' && 'code' in e && 'message' in e) return e as UiError;
  return { code: 'E_UNKNOWN', message: String(e) };
}

async function call<T>(cmd: string, payload?: any): Promise<T> {
  if (!isTauriEnvironment()) {
    throw normalizeError({ code: 'E_NOT_TAURI', message: 'Tauri API not available in browser environment' });
  }

  try {
    const result = await safeInvoke<T>(cmd, payload);
    if (result === null) {
      throw new Error('Tauri command returned null');
    }
    return result;
  }
  catch (e) { throw normalizeError(e); }
}

export const selectWorkspace = (path: string) => call<WorkspaceSummary>('select_workspace', { path });
export const reindex = () => call<IndexSummary>('reindex');
export const listDocuments = (filter?: string) => call<DocMeta[]>('list_documents', { filter });
export const readDocument = (id: string) => call<DocReadResult>('read_document', { id });
export const updateDocument = (id: string, payload: any) => call<SaveResult>('update_document', { input: { id, payload } });
export const compose = (req: ComposeRequest) => call<PromptBundle>('compose', { req });
export const setWatchPaths = (paths: string[]) => call<string[]>('set_watch_paths', { paths });
