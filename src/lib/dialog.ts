interface FileFilter {
  name: string;
  extensions: string[];
}

interface PickerOptions {
  title?: string;
  filters?: FileFilter[];
}

interface SavePickerOptions {
  title?: string;
  defaultPath?: string;
  filters?: FileFilter[];
}

export async function openFilePicker(
  options?: PickerOptions,
): Promise<string | null> {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({
      title: options?.title,
      filters: options?.filters,
    });
    return typeof selected === "string" ? selected : null;
  } catch (e) {
    console.error("Dialog plugin not available:", e);
    return null;
  }
}

export async function openDirectoryPicker(options?: {
  title?: string;
}): Promise<string | null> {
  try {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({ directory: true, title: options?.title });
    return typeof selected === "string" ? selected : null;
  } catch (e) {
    console.error("Dialog plugin not available:", e);
    return null;
  }
}

export async function saveFilePicker(
  options?: SavePickerOptions,
): Promise<string | null> {
  try {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const selected = await save({
      title: options?.title,
      defaultPath: options?.defaultPath,
      filters: options?.filters,
    });
    return typeof selected === "string" ? selected : null;
  } catch (e) {
    console.error("Dialog plugin not available:", e);
    return null;
  }
}
