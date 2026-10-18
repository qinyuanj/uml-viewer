local E=require('uml.edn')
local function same(text,value) assert.is_true(E.equal(E.parse(text),value),E.encode(value)) end

describe('language graph to policy IR',function()
  it('dispatches a registered supported-language scan and reports an unknown language',function()
    local graph=require('uml.graph'); local policy=require('uml.domain.policy')
    assert.has_error(function() graph.scan_project(E.keyword('cobol'),'src',E.map()) end,'no LanguageGraph for :cobol')
    graph.register(E.keyword('lua'),function(root,opts)
      assert.are.equal('lua-fixture/src',root)
      assert.are.equal('calc',E.get(opts,E.keyword('prefix')))
      return E.parse('{:classes [{:id :calc :name "Calc" :lang :lua}] :edges []}')
    end)
    local doc=policy.apply_policy(E.map(),graph.scan_project(E.keyword('lua'),'lua-fixture/src',E.parse('{:prefix "calc"}')))
    same('[{:id :calc :name "Calc" :lang :lua}]',E.get(doc,E.keyword('classes')))
  end)
  it('merges supported-language scans with real ownership, command links and structural deduplication',function()
    local graph=require('uml.graph'); local policy=require('uml.domain.policy')
    local scans=E.parse([=[[
      {:classes [{:id :web :name "Web" :lang :typescript} {:id :rust :foreign true} {:id :external :name "First" :foreign true}]
       :edges [{:from :web :to :rust :kind :dependency}] :invokes {:web ["read_text" "allow_book" "missing"]}}
      {:classes [{:id :rust :name "Rust" :lang :rust} {:id :external :name "Later" :foreign true}]
       :edges [{:from :web :to :rust :kind :dependency}] :commands {:rust ["read_text" "allow_book"]} :invokes {:rust ["read_text"]}}
    ]]=])
    local doc=policy.apply_policy(E.parse('{:foreign [:external]}'),graph.merge_scans(scans))
    same('[{:id :web :name "Web" :lang :typescript} {:id :rust :name "Rust" :lang :rust} {:id :external :name "external" :foreign true}]',E.get(doc,E.keyword('classes')))
    same('[{:from :web :to :rust :kind :dependency}]',E.get(doc,E.keyword('edges')))
    assert.has_error(function() graph.merge_scans(E.parse('[{:classes [{:id :main}]} {:classes [{:id :main}]}]')) end,'duplicate class id: :main')
  end)
  it('lets injected Lua scanners retain namespace identity, names, paths and member dependencies',function()
    local graph=require('uml.graph'); local policy=require('uml.domain.policy')
    graph.register(E.keyword('lua'),function()
      local cid=graph.id_of('demo.calc.nodeFs','demo')
      local c=E.map({{E.keyword('id'),cid},{E.keyword('name'),graph.module_name(cid)},{E.keyword('ns'),graph.ns_join('demo','calc.nodeFs')},{E.keyword('file'),graph.relative_path('C:\\project\\src\\nodeFs.lua','C:\\project')},{E.keyword('requires'),E.vector{E.keyword('socket.http')}}})
      local edges=graph.member_edges(c,E.vector{E.vector{E.keyword('requires'),E.keyword('dependency')}})
      return E.map{{E.keyword('classes'),E.vector{c,graph.foreign_class(E.keyword('socket.http'))}},{E.keyword('edges'),edges}}
    end)
    local doc=policy.apply_policy(E.parse('{:foreign [:socket]}'),graph.scan_project('lua','src'))
    same('[{:id :calc.nodeFs :name "NodeFs" :ns "demo.calc.nodeFs" :file "src/nodeFs.lua" :requires [:socket.http]} {:id :socket :name "socket" :foreign true}]',E.get(doc,E.keyword('classes')))
    same('[{:from :calc.nodeFs :to :socket :kind :dependency}]',E.get(doc,E.keyword('edges')))
  end)
  it('allows scanners to mask comments and quoted dependency decoys before emitting IR',function()
    local graph=require('uml.graph'); local policy=require('uml.domain.policy')
    graph.register('lua',function()
      local source='// require "decoy"\n/* block\ncomment */"escaped\\\" quote" calc'; local out={}; local holes={}; local i=1
      while i<=#source do local c=source:sub(i,i); local nxt=graph.next_char(source,i)
        if graph.line_comment(c,nxt) then i=graph.mask_line_comment(source,out,i)
        elseif graph.block_comment(c,nxt) then i=graph.mask_block_comment(source,out,i)
        elseif c=='"' then local result=graph.copy_span(out,source,holes,i,graph.scan_quoted(source,i,c)); i,holes=result.i,result.holes
        else out[#out+1]=c; i=i+1 end
      end
      local masked=table.concat(out); local classes=E.vector()
      for start,word in masked:gmatch('()([%a_][%w_]*)') do
        local quoted=false; for _,span in ipairs(holes) do if start>=span[1] and start<span[2] then quoted=true end end
        if not quoted then local cid=graph.id_of('demo.'..word,'demo'); classes[#classes+1]=E.map{{E.keyword('id'),cid},{E.keyword('name'),graph.module_name(cid)}} end
      end
      return E.map{{E.keyword('classes'),classes},{E.keyword('edges'),E.vector()}}
    end)
    same('[{:id :calc :name "Calc"}]',E.get(policy.apply_policy(E.map(),graph.scan_project('lua','src')),E.keyword('classes')))
  end)
  it('keeps command ownership last-wins and first foreign identity until policy projection',function()
    local graph=require('uml.graph'); local policy=require('uml.domain.policy')
    local merged=graph.merge_scans(E.parse('[{:classes [{:id :web} {:id :rust} {:id :lua} {:id :socket :name "First" :foreign true}] :invokes {:web ["run"]} :commands {:rust ["run"]}} {:classes [{:id :socket :name "Second" :foreign true}] :commands {:lua ["run"]}}]'))
    same('[{:from :web :to :lua :kind :dependency}]',E.get(policy.apply_policy(E.map(),merged),E.keyword('edges')))
    -- Scanner output is public at the IR handoff, before foreign projection.
    same('{:id :socket :name "First" :foreign true}',E.get(merged,E.keyword('classes'))[4])
  end)
  it('uses scanner namespace boundaries without prefix or path collisions',function()
    local graph=require('uml.graph'); local policy=require('uml.domain.policy')
    graph.register('lua',function()
      local classes=E.vector()
      for _,case in ipairs{{'demo','demo','C:/project/file.lua','C:/project'},{'demolition.a','demo','C:/projectile/a.lua','C:/project'},{'demo.calc.some--name',' ','C:/project','C:/project'}} do
        local cid=graph.id_of(case[1],case[2]); classes[#classes+1]=E.map{{E.keyword('id'),cid},{E.keyword('name'),graph.module_name(cid)},{E.keyword('file'),graph.relative_path(case[3],case[4])}}
      end
      return E.map{{E.keyword('classes'),classes},{E.keyword('edges'),E.vector()}}
    end)
    same('[{:id :demo :name "Demo" :file "file.lua"} {:id :demolition.a :name "A" :file "C:/projectile/a.lua"} {:id :demo.calc.some--name :name "SomeName" :file "C:/project"}]',E.get(policy.apply_policy(E.map(),graph.scan_project('lua','src')),E.keyword('classes')))
  end)
end)
