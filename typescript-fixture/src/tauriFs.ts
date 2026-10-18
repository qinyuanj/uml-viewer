import type { DirEntry, Fs } from "./book";
import { invoke } from "@tauri-apps/api/core";
import "./styles.css";
export const tauriFs: Fs = {
  readText: (path) => invoke("read_text", { path }),
  readDir: (path) => invoke<DirEntry[]>("read_dir", { path }),
};
export function allowBook(root: string): Promise<void> {
  return invoke("allow_book", { root });
}
// invoke("not_a_call")
const note = "invoke(\"hidden\")";
