// Typed wrapper around `@tauri-apps/api/core::invoke` for Promptibrary IPC.
//
// Tauri's `invoke` is generic over the return type but throws plain Error on
// failure. This wrapper rethrows AppError instances (`AppErrorDto`-shaped) so
// callers can `try { … } catch (e) { if (isAppError(e)) … }` cleanly.

import { invoke as tauriInvoke, type InvokeArgs } from "@tauri-apps/api/core";

import { isAppError, type AppErrorDto } from "@/shared/types/ipc";

export type IpcCommand = string;

/**
 * Invoke a Rust IPC command. The return type is whatever the command's
 * `#[tauri::command]` function returns (`Result<T, AppError>` from Rust side
 * resolves to `T` on the JS side, with the error path thrown).
 *
 * If the underlying error is an `AppErrorDto` shape, it is rethrown unchanged.
 * Other failures (network, missing command) are wrapped as `Internal`.
 */
export async function invoke<T>(
  command: IpcCommand,
  args?: InvokeArgs,
): Promise<T> {
  try {
    return await tauriInvoke<T>(command, args);
  } catch (err) {
    if (isAppError(err)) {
      throw err;
    }
    const fallback: AppErrorDto = {
      kind: "Internal",
      message: err instanceof Error ? err.message : String(err),
      details: { command },
    };
    throw fallback;
  }
}
