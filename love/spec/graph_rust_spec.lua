local E=require('uml.edn')
local function rust() return require('uml.rust_language.graph_rust') end
local function get(m,k) return E.get(m,E.keyword(k)) end
local function equal(expected,actual) assert.is_true(E.equal(E.parse(expected),actual),E.encode(actual)) end
local function snapshots(t) local fs={}; for p,s in pairs(t) do fs[#fs+1]={path='C:/project/'..p,source=s} end; return fs end
local function scan(t) return rust().scan('C:/project/src',E.parse('{:prefix "demo" :ns-prefix "demo.rust"}'),snapshots(t),'C:/project') end
describe('Rust reachable graph',function()
  it('resolves file modules, directory modules, self, super and trait bindings',function()
    local g=scan{['src/lib.rs']='mod proto;\nmod rec;\nuse self::proto::Q;\nimpl proto::Q for u8 {}',['src/proto/mod.rs']='mod inner;',['src/proto/inner.rs']='pub fn nested() {}',['src/rec.rs']='mod inner;\nuse crate::proto::Q;\nimpl Q for S {}',['src/rec/inner/mod.rs']='use super::super::proto::Q;\nuse super::Q;\nimpl crate::proto::Q for S {}'}
    equal('[{:from :rust :to :rust.proto :kind :dependency} {:from :rust :to :rust.rec :kind :dependency} {:from :rust :to :rust.proto :kind :implements} {:from :rust.proto :to :rust.proto.inner :kind :dependency} {:from :rust.rec :to :rust.rec.inner :kind :dependency} {:from :rust.rec :to :rust.proto :kind :dependency} {:from :rust.rec :to :rust.proto :kind :implements} {:from :rust.rec.inner :to :rust.rec :kind :dependency} {:from :rust.rec.inner :to :rust.proto :kind :implements}]',get(g,'edges'))
  end)
  it('prefers sibling rs over mod.rs and ignores root mod.rs, inline modules and tests',function()
    local g=scan{['src/lib.rs']='mod chosen;\nmod tests;\nmod inline {}',['src/chosen.rs']='',['src/chosen/mod.rs']='pub fn wrong() {}',['src/tests.rs']='',['src/inline.rs']='',['src/orphan.rs']=''}
    assert.equals(2,#get(g,'classes')); assert.equals('chosen.rs',get(get(g,'classes')[1],'name'))
    equal('[]',get(scan{['src/mod.rs']='pub fn nope() {}'},'classes'))
  end)
  it('uses only explicit lib names and gives root Cargo precedence over parent',function()
    local fs={['Cargo.toml']='[lib]\nname="demo_lib"',['src/Cargo.toml']='[package]\nname="demo_lib"',['src/lib.rs']='',['src/main.rs']='fn main() { demo_lib::run(); }'}
    equal('[]',get(scan(fs),'edges'))
    fs['src/Cargo.toml']='[lib]\n name="demo_lib"'; equal('[]',get(scan(fs),'edges'))
    fs['src/Cargo.toml']='[lib]\nname="demo_lib"\n[other]\n'; equal('[{:from :rust.main :to :rust :kind :dependency}]',get(scan(fs),'edges'))
  end)
  it('keeps string crate mentions, drops self edges and preserves project foreign collisions',function()
    local g=rust().scan('C:/project/src',E.parse('{:prefix "demo" :ns-prefix "demo"}'),snapshots{['Cargo.toml']='[lib]\nname="demo_lib"',['src/lib.rs']='mod serde;\nuse serde::Serialize;\nuse serde::Other;',['src/serde.rs']='',['src/main.rs']='let s="demo_lib::run"; // demo_lib::ignored'},'C:/project')
    assert.equals(3,#get(g,'classes')); equal('[{:from :demo :to :serde :kind :dependency} {:from :main :to :demo :kind :dependency}]',get(g,'edges'))
  end)
  it('links scanner command ownership to a caller during polyglot merge',function()
    local g=scan{['src/lib.rs']='#[tauri::command]\nfn allow_book() {}'}
    local merged=require('uml.graph').merge_scans{E.parse('{:classes [{:id :frontend :name "Frontend"}] :edges [] :invokes {:frontend ["allow_book"]}}'),g}
    equal('[{:from :frontend :to :rust :kind :dependency}]',get(merged,'edges'))
  end)
  it('matches the independent JVM Rust fixture IR golden',function()
    local function read(path) local f=assert(io.open(path,'rb')); local s=f:read('*a'); f:close(); return s end
    local files={}; for _,p in ipairs{'Cargo.toml','src/lib.rs','src/main.rs','src/ignored.rs','src/proto/mod.rs','src/proto/inner/mod.rs','src/rec.rs'} do files[#files+1]={path='C:/fixture/'..p,source=read('rust-fixture/'..p)} end
    local G=require('uml.graph'); local IR=require('uml.application.ir_generator'); local previous=G.lookup('rust')
    G.register('rust',function(_,opts) return rust().scan('C:/fixture/src',opts,files,'C:/fixture') end)
    local adapter={read=function() return read('rust-fixture/policy.edn') end,write=function(_,text)
      assert.is_true(E.equal(E.parse(read('fixtures/jvm-goldens/rust.ir.edn')),E.parse(text)),text)
    end}
    local ok,err=pcall(IR.generate,nil,'policy.edn','out.edn',adapter)
    G.register('rust',previous); assert(ok,err)
  end)
  it('links reachable modules, the binary and commands using parent Cargo metadata',function()
    local g=scan{['Cargo.toml']='[lib]\nname = "demo_lib"\n',['src/lib.rs']='use tauri::Manager;\nmod proto;\n#[tauri::command]\nfn read_text() {}\npub fn run() {}',['src/proto.rs']='pub trait Q {}',['src/main.rs']='fn main() { demo_lib::run(); }',['src/ignored.rs']='pub fn nope() {}'}
    equal('[{:from :rust :to :rust.proto :kind :dependency} {:from :rust :to :tauri :kind :dependency} {:from :rust.main :to :rust :kind :dependency}]',get(g,'edges'))
    equal('{:rust ["read_text"]}',get(g,'commands'))
    equal('[{:id :rust :ns "demo.rust" :name "lib.rs" :lang :rust :file "src/lib.rs" :ops [{:name "read_text" :text "read_text"} {:name "run" :text "run"}]} {:id :rust.main :ns "demo.rust.main" :name "main.rs" :lang :rust :file "src/main.rs"} {:id :rust.proto :ns "demo.rust.proto" :name "proto.rs" :lang :rust :file "src/proto.rs"} {:id :tauri :name "tauri" :ns "tauri" :foreign true}]',get(g,'classes'))
  end)
end)
describe('Rust module surface',function()
  it('accepts source and Cargo metadata without applying other language filters',function()
    for _,p in ipairs{'lib.rs','tests/kept.rs','.hidden/kept.rs','target/kept.rs','Cargo.toml','nested/Cargo.toml'} do assert.is_true(rust().accepts(p),p) end
    for _,p in ipairs{'lib.ts','cargo.toml','Cargo.toml.bak'} do assert.is_false(rust().accepts(p),p) end
    assert.same({'C:/project/src/Cargo.toml','C:/project/Cargo.toml'},rust().metadata_paths('C:/project/src'))
  end)
  it('preserves command attributes, restricted visibility and generic trait implementations',function()
    local s=rust().read_module('#[ tauri :: command(rename_all="snake_case") ]\npub(crate) async fn command() {}\n#[tauri::command]\nfn command() {}\npub(crate) fn restricted() {}\nimpl<T> crate::proto::Q for S<T> {}\nimpl Inherent {}\nuse crate::proto::Q as Alias;\nimpl Alias for S {}')
    equal('["command"]',get(s,'commands')); equal('["command"]',get(s,'ops'))
    equal('["crate::proto::Q" "Alias"]',get(s,'impls')); equal('{"Q" ["crate" "proto" "Q"]}',get(s,'bindings'))
  end)
  it('keeps comment masking and literal holes faithful including malformed literals',function()
    local s=rust().read_module('/* outer /* nested */ pub fn visible() {} */\n// pub fn hidden() {}\nlet x = "\\\" pub fn hidden() {}";\nlet raw = r#"pub fn hidden() {}"#;\nlet c = \'x\';\npub fn after() {}\nlet unfinished="pub fn hidden() {}')
    equal('["visible" "after"]',get(s,'ops'))
  end)
  it('preserves masking, visibility and whitespace matching quirks',function()
    local s=rust().read_module('\n\nuse crate::proto::{Q, R};\n\nmod proto;\nmod tests;\n// pub fn nope() {}\nlet s = "pub fn fake() {}";\npub(crate) fn hidden() {}\npub async fn run() {}\n#[tauri::command]\n#[other]\nfn ignored() {}\n')
    equal('["crate::proto::{Q, R}"]',get(s,'uses')); equal('["proto"]',get(s,'mods')); equal('["run"]',get(s,'ops')); equal('[]',get(s,'commands'))
    equal('{"proto" ["crate" "proto"]}',get(s,'bindings'))
  end)
  it('keeps escaped characters and lifetimes from hiding subsequent functions',function()
    local s=rust().read_module("pub fn a() { let nl = '\\n'; let q = '\\''; }\npub fn b() { let c = '\\\\'; let hex = '\\x41'; }\npub fn c() { let open = '\\")
    equal('["a" "b" "c"]',get(s,'ops'))
  end)
  it('reads uses, modules, commands and public functions',function()
    local s=rust().read_module('use serde::Serialize;\nuse std::fs;\nmod proto;\n#[tauri::command]\nfn read_text() {}\npub fn run() {}\nfn hidden() {}')
    equal('["serde::Serialize" "std::fs"]',get(s,'uses'))
    equal('["proto"]',get(s,'mods')); equal('["read_text"]',get(s,'commands')); equal('["read_text" "run"]',get(s,'ops'))
  end)
end)
