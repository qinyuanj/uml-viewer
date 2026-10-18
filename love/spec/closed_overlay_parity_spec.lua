local E=require('uml.edn')
local G=require('uml.domain.geom')
local O=require('uml.application.overlay')
local g=G.get
local function snapshot(path,text,mtime) return {path=path,text=text,mtime=mtime,size=#text} end
local function names(c) local r={}; for _,op in ipairs(g(c,'ops') or {}) do r[#r+1]=g(op,'name') end; return r end

describe('closed mixed namespace overlay parity',function()
  it('joins TypeScript modules and Rust crate metrics without leaking build or root metrics to main',function()
    local doc=E.parse('{:hierarchical true :prefix "bookwriter" :classes [{:id :model :ns "bookwriter.model" :lang :typescript} {:id :pdf :ns "bookwriter.pdf" :lang :typescript} {:id :rust :ns "bookwriter.rust" :lang :rust :file "src-tauri/src/lib.rs"} {:id :rust.main :ns "bookwriter.rust.main" :lang :rust :file "src-tauri/src/main.rs"}]}')
    local cs=g(O.apply(doc,{
      snapshot('.metrics/crap.edn',[[{:entries [
        {:namespace "model" :name "slugify" :complexity 2 :coverage 100 :crap 2}
        {:namespace "pdf" :name "styled" :complexity 4 :coverage 100 :crap 4}
        {:namespace "pdf.Layout" :name "figure" :complexity 8 :coverage 100 :crap 8}
        {:namespace "bookwriter" :name "read_text" :complexity 1 :coverage 100 :crap 1}
        {:namespace "bookwriter" :name "main" :complexity 1 :coverage 0 :crap 2}
        {:namespace "bookwriter::build" :name "main" :complexity 1 :coverage nil :crap nil}]}]],1),
      snapshot('.metrics/mutate/model.edn','{:namespace "model" :forms [{:id "defn/slugify" :killed 1 :survived 0 :uncovered 0 :sites 1}]}',1)
    }),'classes')
    assert.are.equal(2,g(cs[1],'cc')); assert.are.equal(1,g(cs[1],'killed')); assert.are.equal(1,g(cs[1],'sites')); assert.are.same({'slugify'},names(cs[1]))
    assert.are.equal(12,g(cs[2],'cc')); assert.are.same({'styled','figure'},names(cs[2]))
    assert.are.equal(2,g(cs[3],'cc')); assert.are.same({'read_text','main'},names(cs[3]))
    assert.is_nil(g(cs[4],'crap')); assert.are.same({},names(cs[4]))
  end)
  for _,times in ipairs{{2000,1000,1},{2000,3000,2}} do
    it('keeps the newer explicit or source-derived mutation snapshot at source time '..times[2],function()
      local doc=E.parse('{:hierarchical true :classes [{:id :config :ns "demo.config"}]}')
      local c=g(O.apply(doc,{
        snapshot('.metrics/mutate/demo.config.edn','{:namespace "demo.config" :forms [{:id "defn/worse" :killed 1 :survived 9}]}',times[1]),
        snapshot('.metrics/mutate/demo/config.edn','{:source "src/demo/config.lua" :forms [{:id "defn/pool" :killed 2 :survived 0}]}',times[2])
      }),'classes')[1]
      assert.are.equal(times[3],g(c,'killed'))
      assert.are.same(times[3]==1 and {'worse'} or {'pool'},names(c))
    end)
  end
  it('derives a Lua namespace with underscores from its source path',function()
    local doc=E.parse('{:hierarchical true :classes [{:id :config :ns "demo.my_config"}]}')
    local c=g(O.apply(doc,{snapshot('.metrics/mutate/config.edn','{:source "src/demo/my_config.lua" :forms [{:id "defn/pool" :killed 2 :survived 0}]}',1)}),'classes')[1]
    assert.are.equal(2,g(c,'killed')); assert.are.same({'pool'},names(c))
  end)
end)
