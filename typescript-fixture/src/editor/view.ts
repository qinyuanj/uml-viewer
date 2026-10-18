import MarkdownIt from "markdown-it";
export type { Fs } from "../book";
export * from "../book";
const q = /"/g;
const s = "import { a } from \"./nope\";";
/* import { b } from "./nope"; */
export function show(): void {}
export { show as render };
export class Page {}
export enum Kind { A }
