import { wordCount } from "./model";
export interface Fs { readText(path: string): Promise<string>; }
export interface DirEntry { name: string; }
export async function loadBook(fs: Fs): Promise<string> {
  return String(wordCount(""));
}
