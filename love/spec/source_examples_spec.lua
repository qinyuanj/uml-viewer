local Source=require('uml.main.source')
local cases={
 {'ts-export','typescript','export function loadBook(): string {\n  return "";\n}\n','loadBook',1},
 {'ts-missing','typescript','export const answer = 1;\n','missing'},
 {'ts-module','typescript','export function wordCount(): number { return 0; }\n'},
 {'rs-tauri-line','rust','#[tauri::command]\nfn read_text(path: String) -> String { path }\npub fn run() {}\n','read_text',1,'#[tauri::command]\nfn read_text(path: String) -> String { path }'},
 {'rs-public-line','rust','#[tauri::command]\nfn read_text(path: String) -> String { path }\npub fn run() {}\n','run',3},
 {'rs-tauri-extract','rust','#[tauri::command]\nfn read_text(path: String) -> String { path }\npub fn run() {}\n','read_text',1,'#[tauri::command]\nfn read_text(path: String) -> String { path }'},
 {'rs-public-file','rust','pub fn run() {}\n','run',1},
 {'py-function','python','class Animal:\n    pass\n\ndef walk(animal):\n    return animal\n','walk',4},
 {'py-missing','python','def walk():\n    return 1\n','missing'},
 {'py-module','python','def walk():\n    return 1\n'},
 {'lua-table','lua','local M = {}\n\nfunction M.load()\n  return 1\nend\n','M.load',3},
 {'lua-assignment','lua','M.save = function(x)\n  return x\nend\n\nfunction Account:deposit(a)\n  return a\nend\n','M.save',1},
 {'lua-method','lua','M.save = function(x)\n  return x\nend\n\nfunction Account:deposit(a)\n  return a\nend\n','Account:deposit',5},
 {'lua-missing','lua','function M.load() end\n','M.missing'},
 {'ts-priority','typescript','function pick() {}\nexport function pick() {}\n','pick',2},
 {'rs-priority','rust','pub fn pick() {}\n#[tauri::command]\nfn pick() {}\n','pick',2},
 {'py-priority','python','def pick(): pass\nasync def pick(): pass\n','pick',2},
 {'lua-priority','lua','M.pick = function() end\nfunction M.pick() end\n','M.pick',2},
}
describe('prepared source contract examples (18 independent literals)',function()
  for _,case in ipairs(cases) do
    it(case[1],function()
      local id,lang,body,name,line,span=case[1],case[2],case[3],case[4],case[5],case[6]
      local ident={lang=lang,file='f/example',ns='app.some_module',name=name}
      local result=Source.member_source(ident,{is_file=function() return true end,read=function() return body end})
      if name and not line then assert.is_nil(result)
      else
        assert.same({file='f/example',body=body,lang=lang,line=line,title='f/example'..(name and ':'..line or '')},result)
        if span then assert.equal(span,Source.lookup(lang).extract(body,ident)) end
      end
    end)
  end
end)
