// Tauri 환경 감지 및 안전한 API 호출을 위한 유틸리티
import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import { listen as tauriListen } from '@tauri-apps/api/event';

// Re-export Tauri functions for consistent imports
export { tauriInvoke as invoke, tauriListen as listen };

export function isTauriEnvironment(): boolean {
  return typeof window !== 'undefined' && '__TAURI__' in window;
}

export async function safeInvoke<T>(command: string, payload?: any): Promise<T | null> {
  if (!isTauriEnvironment()) {
    console.warn(`Tauri command '${command}' called in non-Tauri environment`);
    return null;
  }
  
  try {
    return await tauriInvoke<T>(command, payload);
  } catch (error) {
    console.error(`Failed to invoke Tauri command '${command}':`, error);
    throw error;
  }
}

export async function safeListen(event: string, handler: (event: any) => void): Promise<(() => void) | null> {
  if (!isTauriEnvironment()) {
    console.warn(`Tauri event listener '${event}' registered in non-Tauri environment`);
    return null;
  }
  
  try {
    const unlisten = await tauriListen(event, handler);
    return unlisten;
  } catch (error) {
    console.error(`Failed to listen to Tauri event '${event}':`, error);
    throw error;
  }
}