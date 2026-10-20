-- Pure paint plan for the separate detail window: one item per Detail row with
-- its font role, text color, fixed right-aligned metric cells, quality swatch,
-- Cov / kill-rate bars and alternating stats backgrounds. Colors are theme RGB
-- triples following the 4e69e34 detail semantics; the adapter only paints.
local T=require('uml.domain.theme')
local C=require('uml.domain.config')
local S=require('uml.application.strings')
local Detail=require('uml.application.detail')
local M={}
M.pad,M.swatch_w,M.swatch_gap=16,10,4
local gold_kinds={crap=true,heading=true,['group-header']=true,['col-header']=true}
local ink_kinds={name=true,module=true,field=true,rel=true,stats=true}
function M.row_color(row)
  if row.private then return T.colors.muted end
  if gold_kinds[row.kind] then return T.colors.gold end
  if ink_kinds[row.kind] or row.module then return T.colors.ink end
  return T.colors.muted
end
function M.cell_color(row,id)
  if id=='cov' then return T.coverage(row.coverage) end
  if id=='crap' then return T.stroke(C.crap_grade(row['crap-n'])) end
  if id=='killed' then return T.colors.white end
  if id=='survived' or id=='uncovered' then return (row[id] or 0)>0 and T.colors.violation or T.colors.good end
  return T.colors.muted
end
local function kill_ratio(row)
  local total=(row.killed or 0)+(row.survived or 0)+(row.uncovered or 0)
  if total>0 then return (row.killed or 0)/total end
end
-- A member's quality: the averaged CRAP / mutation grade, each missing metric
-- entering as grade 1 (config), so an unmeasured member sits at the red end.
function M.swatch(row)
  return T.stroke(C.combined_grade(C.crap_grade(row['crap-n']),C.mutation_grade(kill_ratio(row))))
end
function M.rows(rows,width)
  local out={}; local cols=Detail.column_layout(width); local groups=Detail.group_layout(width); local stats=0
  local member_right=cols[1].left-10
  for _,row in ipairs(rows or {}) do
    local it={kind=row.kind,source=row,y=row.y,h=row.h,text=row.text,color=M.row_color(row),
      font=row.kind=='name' and 'title' or 'code',x=M.pad,clip_right=member_right,cells={},bars={},stripe=false,
      clickable=(row.module or row['op-name']~=nil or row.kind=='rel') and true or false}
    if row.kind~='stats' and row.kind~='col-header' then it.clip_right=nil end
    if row.kind=='group-header' then
      it.text=nil
      it.cells={{id='crap',text=S.group_crap,left=groups[1].left,right=groups[1].right,align='center',color=T.colors.gold},
        {id='mutation',text=S.group_mutation,left=groups[2].left,right=groups[2].right,align='center',color=T.colors.gold}}
    elseif row.kind=='col-header' then
      it.text=S.member
      for _,col in ipairs(cols) do it.cells[#it.cells+1]={id=col.id,text=col.label,left=col.left,right=col.right,align='right',color=T.colors.gold,tip=S.column_tips[col.id]} end
    elseif row.kind=='stats' then
      stats=stats+1; it.stripe=stats%2==0; it.swatch=M.swatch(row); it.x=M.pad+M.swatch_w+M.swatch_gap
      for i,col in ipairs(cols) do
        if (i<=3 or not row['mut-note']) and row[col.key]~=nil then
          it.cells[#it.cells+1]={id=col.id,text=row[col.key],left=col.left,right=col.right,align='right',color=M.cell_color(row,col.id)}
        end
      end
      if row['mut-note'] then it.cells[#it.cells+1]={id='mut-note',text=row['mut-note'],left=groups[2].left,right=groups[2].right,align='center',color=T.colors.muted}
      else
        local ratio=kill_ratio(row)
        if ratio then it.bars[#it.bars+1]={id='kill',left=groups[2].left,right=groups[2].right,fraction=ratio,color=T.stroke(C.mutation_grade(ratio))} end
      end
      if row.coverage~=nil then it.bars[#it.bars+1]={id='cov',left=cols[3].left,right=cols[3].right,fraction=row.coverage,color=T.coverage(row.coverage)} end
    end
    out[#out+1]=it
  end
  return out
end
return M
