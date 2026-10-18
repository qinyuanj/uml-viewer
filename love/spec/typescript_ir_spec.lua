local E=require('uml.edn')
local IR=require('uml.application.ir_generator')
local function get(m,k) return E.get(m,E.keyword(k)) end
local function document(sources,policy)
  local scanner=require('uml.typescript_language.graph_typescript')
  local files={}; for path,source in pairs(sources) do files[#files+1]={path='/project/src/'..path,source=source} end
  return IR.document(function(_,opts) return scanner.scan('/project/src',opts,files,'/project') end,E.parse(policy or '{:prefix "bookwriter" :hierarchical true}'))
end
local function class(doc,id) for _,c in ipairs(get(doc,'classes')) do if E.equal(get(c,'id'),E.keyword(id)) then return c end end end
local function names(c) local r={}; for _,op in ipairs(get(c,'ops') or {}) do r[#r+1]=get(op,'name') end; return r end
local function edge(doc,from,to,kind) for _,e in ipairs(get(doc,'edges')) do if E.equal(e,E.parse('{:from :'..from..' :to :'..to..' :kind :'..kind..'}')) then return true end end; return false end

describe('TypeScript IR generation',function()
  it('filters tests and assets, folds index modules and resolves relative re-exports in order',function()
    local doc=document({
      ['index.ts']='export { run as launch } from "./editor";\nimport "./theme.css";\nimport "./missing";',
      ['editor/index.ts']='import Default, { type Fs as FileSystem } from "../book";\nexport const adapter: FileSystem = {};\nexport { Default as run };',
      ['book.ts']='export interface Fs {}',
      ['preview.mtsx']='import MarkdownIt from "markdown-it";\nimport * as api from "@scope/pkg/api";\nexport default class Preview {}',
      ['book.test.ts']='export class Wrong {}', ['book.spec.cjsx']='', ['types.d.ts']='',
      ['node_modules/a.ts']='', ['dist/a.ts']='', ['target/a.ts']='', ['.hidden/ok.ts']='export const yes = 1;',
    },'{:prefix "bookwriter" :hierarchical true :foreign [:markdown-it :at.scope.pkg.api]}')
    assert.same({'launch'},names(class(doc,'bookwriter')))
    assert.same({'adapter','run'},names(class(doc,'editor')))
    assert.equals('bookwriter.editor',get(class(doc,'editor'),'ns'))
    assert.is_true(edge(doc,'bookwriter','editor','dependency'))
    assert.is_true(edge(doc,'editor','book','implements'))
    assert.is_true(edge(doc,'preview','markdown-it','dependency'))
    assert.is_true(edge(doc,'preview','at.scope.pkg.api','dependency'))
    assert.is_not_nil(class(doc,'.hidden.ok'))
    assert.equals(7,#get(doc,'classes'))
  end)
  it('links only real literal invoke calls to command owners in a combined IR',function()
    local G=require('uml.graph')
    local TS=require('uml.typescript_language.graph_typescript')
    local source=[==[
const ignored = `invoke("hidden")`;
const pattern = /invoke("regex")/;
// invoke("comment")
invoke<Nested<Array<T>>>("read_text"); invoke("read_text");
invoke <T> ("spaced"); invoke(`template`); invoke(variable);
obj.invoke('allow_book'); $invoke('prefixed'); invokeSuffix('suffix');
]==]
    local doc=IR.document(function(_,opts)
      return G.merge_scans({TS.scan('/project/src',opts,{{path='/project/src/tauriFs.ts',source=source}},'/project'),E.parse('{:classes [{:id :rust :name "Rust" :ns "bookwriter.rust"}] :edges [] :commands {:rust ["read_text" "allow_book"] :hidden ["hidden" "regex" "comment" "spaced" "template" "prefixed" "suffix"]}}')})
    end,E.parse('{:prefix "bookwriter" :hierarchical true}'))
    assert.is_true(edge(doc,'tauriFs','rust','dependency'))
    assert.equals(1,#get(doc,'edges'))
  end)
  it('ignores comments, templates and regex literals while retaining real exports',function()
    local doc=document({['view.ts']=[==[
/* export function wrong() {} */
const note = `
export function hidden() {}
`;
const regex = /"/g;
export function render(): string { return ""; }
export { render as show };
export class Page {}
export enum Kind { A }
export type { Secret } from './missing';
]==]})
    assert.same({'render','show','Page','Kind'},names(class(doc,'view')))
  end)
  it('scans imports and exported annotations into module dependencies and implements',function()
    local doc=document({['book.ts']='export interface Fs { read(): void; }', ['nodeFs.ts']='import type { Fs } from "./book";\nexport function nodeFs(): Fs { return null; }'})
    assert.same({'nodeFs'},names(class(doc,'nodeFs')))
    assert.equals('NodeFs',get(class(doc,'nodeFs'),'name'))
    assert.equals('bookwriter.nodeFs',get(class(doc,'nodeFs'),'ns'))
    assert.equals('src/nodeFs.ts',get(class(doc,'nodeFs'),'file'))
    assert.is_true(edge(doc,'nodeFs','book','implements'))
  end)
end)
