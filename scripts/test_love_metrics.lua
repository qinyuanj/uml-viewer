-- Runs only in a newly launched native viewer. Ported from the retired
-- PowerShell wrapper: a metric-only edit of equal byte length (crap 1 -> 9)
-- must still reload, because the stamp carries nanoseconds and the size is
-- unchanged, while the IR file itself is never touched.
local evidence=os.getenv('UML_VIEWER_EVIDENCE')
local function read(path)
  local f=io.open(path,'rb'); if not f then return nil end
  local text=f:read('*a'); f:close(); return text
end
local function write(path,text)
  local f=assert(io.open(path,'wb')); f:write(text); f:close()
end
local function mtime(path)
  local lfs=require('lfs'); local a=lfs.attributes(path); return a and a.modification
end
local injected=false
return function(frame,state)
  if frame<3 then return end
  local diagram=state.path
  local root=diagram:match('^(.*)/[^/]+$') or '.'
  local metric=root..'/.metrics/crap.edn'
  if not injected then
    local before=read(evidence..'/scene-1.edn')
    assert(before and before:find(':mu 1[ }]'),'first scene lacks the crap-1 summary')
    local text=assert(read(metric),'metric snapshot missing: '..metric)
    local edited=text:gsub(':crap 1',':crap 9')
    assert(#edited==#text,'the metric edit must keep the byte length')
    local original=mtime(diagram)
    write(metric,edited)
    injected={diagram_mtime=original}
    return
  end
  local after=read(evidence..'/scene-2.edn')
  if not after then return end
  local before=read(evidence..'/scene-1.edn')
  assert(before~=after,'a metric-only edit must publish a new scene')
  assert(after:find(':mu 9[ }]'),'reloaded scene did not pick up crap 9')
  assert(after:find(':strokes',1,true),'edge renderer strokes missing')
  assert(after:find(':tip',1,true),'edge renderer tip missing')
  assert(mtime(diagram)==injected.diagram_mtime,'the IR file must not be rewritten by a metric reload')
  local f=assert(io.open(evidence..'/metrics-pass.txt','wb'))
  f:write('PASS metric-only reload with an equal-byte-length snapshot; IR untouched\n'); f:close()
  love.event.quit(0)
end
