import { isTauriEnvironment } from './tauri-utils';

export async function openDirectory(): Promise<string | null> {
  if (!isTauriEnvironment()) {
    console.warn('Directory dialog not available in browser environment');
    return null;
  }

  try {
    const { open } = await import('@tauri-apps/plugin-dialog');
    const result = await open({ 
      directory: true, 
      multiple: false 
    });
    return typeof result === 'string' ? result : null;
  } catch (error) {
    console.error('Failed to open directory dialog:', error);
    throw error;
  }
}