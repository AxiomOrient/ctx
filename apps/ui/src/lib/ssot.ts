import { writable } from 'svelte/store';
import type { DocMeta, DocReadResult, PromptBundle } from './types';

export type AppState = {
  workspaceRoot: string | null;
  workspacePath: string | null;
  documents: DocMeta[];
  selectedDoc: DocMeta | null;
  docView: DocReadResult | null;
  promptInput: string;
  promptOutput: PromptBundle | null;
  busy: boolean;
  rightDockMode: 'prompt' | 'ontology';
  showJsonMeta: boolean;
  editMode: boolean;
  settingsOpen: boolean;
  theme: 'system' | 'light' | 'dark';
  errorObj: { code: string; message: string; data?: any } | null;
  showErrorJson: boolean;
};

const initialState: AppState = {
  workspaceRoot: null,
  workspacePath: null,
  documents: [],
  selectedDoc: null,
  docView: null,
  promptInput: '',
  promptOutput: null,
  busy: false,
  rightDockMode: 'prompt',
  showJsonMeta: false,
  editMode: false,
  settingsOpen: false,
  theme: 'system',
  errorObj: null,
  showErrorJson: false,
};

/**
 * The single source of truth for the entire application state.
 * 
 * It is a standard Svelte `writable` store. 
 * - To update, use the `appStore.update(s => ({ ...s, ...newState }))` pattern.
 * - To get a non-reactive value, use `get(appStore)` from 'svelte/store'.
 * - To use reactively in components, use the `$appStore` syntax.
 */
export const appStore = writable<AppState>(initialState);
