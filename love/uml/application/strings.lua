-- The single Chinese UI string table. Terms follow GLOSSARY.md; metric and tool
-- names (CRAP, CC, Cov, mutation, killed, survived, uncovered, IR) and key names
-- stay English. EDN keywords, mail commands, stderr, companion messages, IR
-- warnings, font assertions and the window title are not UI copy and never
-- live here. Every character in this table is added to the font atlas.
local M={}
M.real_diagram='源码图'
M.inspector='检查器'
M.tree_and_proposals='命名空间树 / 提案'
M.new_proposal='新建提案'
M.declutter='精简视图'
M.rename='重命名'
M.delete='删除'
M.proposal_name='提案名称'
M.rename_proposal='重命名提案'
M.cancel='取消'
M.p_hint='P：返回命名空间树'
M.proposals_label='提案'
M.class_count='类数：'
M.package_crap='Crap μ %s max %s σ %s'
M.package_mutation='%d killed / %d survived'
M.package_uncovered='   %d uncovered'
M.no_data='无数据'
M.select_class='单击类或命名空间查看摘要；双击打开详情卡片。'
M.regen='重新生成 IR'
M.close_source='关闭源码'
M.close_detail='关闭详情'
M.back='< 返回'
M.proposal_banner='提案（未在源码中实例化）'
M.performance='性能：%d FPS · 帧耗时 %.2f ms'
M.help=[[单击类或命名空间查看卡片。
双击命名空间或类进入下一层。
Esc（或 ←）返回上一层。
滚轮平移；Shift+滚轮横向平移。
Ctrl+滚轮或 Ctrl+/− 以光标为中心缩放。
空白处左键拖拽平移；Ctrl+0 适配全图。
R 重新加载；F3 切换性能显示。
单击上方的源码图或某个提案即可显示。]]
-- Element menu labels; the mail operations themselves stay English.
M.menu={['refresh-crap']='刷新 CRAP',['refresh-mutate']='刷新 mutation',['refresh-mutate-all']='全量 mutation',omit='隐藏'}
-- Class card.
M.level='层级 '
M.package='包  '
M.fields='字段'
M.relationships='关系'
M.no_mutation_sites='---无 mutation 位点---'
M.member='成员'
M.empty_detail='选择成员查看质量数据。'
M.crap_line='Crap μ %s max %s σ %s'
M.crap_single='Crap %s'
M.group_crap='--crap--'
M.group_mutation='--mutation--'
M.columns={crap='Crap',cc='CC',cov='Cov',killed='killed',survived='survived',uncovered='uncovered'}
M.column_tips={crap='CRAP 风险分数：圈复杂度与覆盖率的综合，越低越好',cc='圈复杂度',cov='行覆盖率',
  killed='被测试杀死的变异体数',survived='测试未能杀死的存活变异体数',uncovered='没有测试覆盖的变异体数'}
-- Relationship phrases: outgoing, incoming.
M.phrases={inheritance={'继承','被继承'},implements={'实现','被实现'},association={'关联','被关联'},dependency={'依赖','被依赖'},aggregation={'聚合','被聚合'},composition={'组合','被组合'}}
M.phrase_fallback={'指向','来自'}
-- Non-table glyphs the canvas and cards draw: stereotype guillemets, the
-- abstract marker, crap statistics and arrows.
M.extra_glyphs='«»αμσ←→—…'
local function collect(value,out)
  if type(value)=='string' then out[#out+1]=value
  elseif type(value)=='table' then
    local keys={}; for k in pairs(value) do keys[#keys+1]=tostring(k) end; table.sort(keys)
    for _,k in ipairs(keys) do collect(value[k] or value[tonumber(k)],out) end
  end
end
-- Every string in the table concatenated, for the atlas glyph builder.
function M.glyph_text()
  local out={}; collect(M,out); return table.concat(out)
end
return M
