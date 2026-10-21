-- #114 acceptance driver, loaded by the real LOVE viewer through
-- UML_VIEWER_TEST_DRIVER. It decides nothing itself:
-- * when the harness drops triggers/<n>.edn it calls the same mail API the
--   right-click menu (hooks.mail.menu_action) or the inspector context choice
--   (hooks.mail.request 'context') uses, so the outbox write and the Herdr
--   wake are the product's own path;
-- * every 30 frames it records what the viewer is actually showing
--   (status.edn) and captures the window whenever the shown document changes.
-- Every call is guarded: a probe error would quit the viewer.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local dir=os.getenv('UML_114_TRIGGERS')
local next_index=1
while dir do
  local f=io.open(dir..'/'..next_index..'.edn.done','rb')
  if not f then break end
  f:close(); next_index=next_index+1
end
local last_key
local shots=0
local function log(line)
  if not dir then return end
  local f=io.open(dir..'/driver.log','ab'); if f then f:write(os.date('!%Y-%m-%dT%H:%M:%SZ'),' ',line,'\n'); f:close() end
end
local function read(path)
  local f=io.open(path,'rb'); if not f then return nil end
  local text=f:read('*a'); f:close(); return text
end
local function write(path,text)
  local f=io.open(path..'.tmp','wb'); if not f then return end
  f:write(text); f:close(); os.rename(path..'.tmp',path)
end
local function count(list) return type(list)=='table' and #list or 0 end
local function ids(doc)
  local out={}
  for _,c in ipairs(G.get(doc,'classes') or {}) do out[#out+1]=G.name(G.get(c,'id')) or '?' end
  table.sort(out); return table.concat(out,',')
end
local function status(frame,state)
  local doc=state.document
  local fields={
    ('frame %d'):format(frame),
    ('path %q'):format(tostring(state.path)),
    ('waiting %s'):format(tostring(state.waiting and true or false)),
    ('mail-seen %s'):format(tostring(state.mail_seen)),
    ('title %q'):format(tostring(doc and G.get(doc,'title'))),
    ('classes %q'):format(doc and ids(doc) or ''),
    ('omit %q'):format(doc and E.encode(G.get(doc,'omit') or E.vector()) or ''),
    ('mtime %s'):format(tostring(state.mtime)),
    ('metrics-stamp %q'):format(tostring(state.metrics_stamp)),
    ('context %q'):format(state.discussion_context and E.encode(state.discussion_context) or 'nil'),
    ('mail-status %q'):format(tostring(state.mail_status)),
    ('scene-classes %d'):format(count(state.scene and G.get(state.scene,'classes'))),
  }
  return fields
end
return function(frame,state,hooks)
  if not dir then return end
  if frame%30==0 then
    pcall(function()
      local fields=status(frame,state)
      write(dir..'/status.txt',table.concat(fields,'\n')..'\n')
      local key=table.concat({fields[2],fields[3],fields[4],fields[6],fields[7],fields[8],fields[9]},'|')
      if key~=last_key then
        last_key=key; shots=shots+1
        log('viewer state '..shots..': '..table.concat(fields,'; '))
        local name=dir..'/shot-'..shots..'.png'
        love.graphics.captureScreenshot(function(data) local f=io.open(name,'wb'); if f then f:write(data:encode('png'):getString()); f:close() end end)
      end
    end)
  end
  if frame%10~=0 then return end
  local ok,err=pcall(function()
    local path=dir..'/'..next_index..'.edn'
    local text=read(path)
    if not text then return end
    local trigger=E.parse(text)
    local op=G.name(G.get(trigger,'op'))
    local cmd
    if op=='context' then
      cmd=hooks.mail.request('context',G.get(trigger,'extra'))
    else
      local selection=G.map{kind=E.keyword('class'),id=G.get(trigger,'id')}
      cmd=hooks.mail.menu_action(state,selection,op)
    end
    log(('trigger %d frame %d op %s -> outbox %s'):format(next_index,frame,op,E.encode(cmd)))
    os.rename(path,path..'.done')
    next_index=next_index+1
  end)
  if not ok then log('trigger '..next_index..' error '..tostring(err)); next_index=next_index+1 end
end
