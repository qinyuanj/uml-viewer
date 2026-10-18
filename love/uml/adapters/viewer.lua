-- LOVE callback installation; owns no companion, mailbox or existing viewer resources.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local IR=require('uml.domain.ir')
local D=require('uml.application.document')
local Paths=require('uml.adapters.windows_path')
local Watch=require('uml.adapters.file_watch')
local Boxes=require('uml.adapters.class_boxes')
local Edges=require('uml.adapters.edges')
local Metrics=require('uml.adapters.metrics')
local M={}
local function read(path) local f=assert(io.open(path,'rb')); local s=f:read('*a'); f:close(); return s end
function M.run(args)
  local path=args[1] or 'examples/uml-viewer.edn'
  assert(path:sub(1,1)~='-','usage: uml --runtime love [diagram.edn]')
  path=Paths.canonical(path)
  local dependencies=os.getenv('UML_CIMGUI_DIRECTORY')
  if dependencies then dependencies=dependencies:gsub('\\','/'); package.path=dependencies..'/?.lua;'..dependencies..'/?/init.lua;'..package.path; package.cpath=dependencies..'/?.dll;'..package.cpath end
  local imgui=require('cimgui')
  local metrics=Metrics.new()
  local state,renderer,elapsed,frames= nil,nil,0,0
  local evidence=os.getenv('UML_VIEWER_EVIDENCE')
  local test_seconds=tonumber(os.getenv('UML_VIEWER_TEST_SECONDS'))
  local function publish()
    if evidence then local f=assert(io.open(evidence..'/scene-'..state.revision..'.edn','wb')); f:write(E.encode(state.scene)); f:close() end
  end
  function love.load()
    love.window.setTitle('UML viewer — LOVE')
    love.window.setMode(1100,760,{resizable=true})
    imgui.love.Init(); imgui.love.BuildFontAtlas(); renderer=Boxes.new(imgui)
    local snapshots,stamp=metrics.snapshot(metrics.root(path))
    state=D.new(IR.read_document(read(path)),Watch.mtime(path),nil,nil,snapshots,stamp); publish()
  end
  function love.update(dt)
    elapsed=elapsed+dt; frames=frames+1
    local ok,err=pcall(function()
      local stamp=Watch.mtime(path)
      local snapshots,metrics_stamp=metrics.snapshot(metrics.root(path))
      if stamp~=state.mtime or metrics_stamp~=state.metrics_stamp then
        local next_state,changed,rejected=D.reload(state,read(path),stamp,snapshots,metrics_stamp)
        state=next_state; if changed then publish() elseif rejected then error(rejected) end
      end
    end)
    if not ok then io.stderr:write('Document reload rejected: '..tostring(err)..'\n') end
    imgui.love.Update(dt); imgui.NewFrame()
    if test_seconds and elapsed>=test_seconds then love.event.quit(0) end
  end
  function love.draw()
    love.graphics.clear(.09,.10,.12,1)
    local width,height=love.graphics.getWidth(),love.graphics.getHeight()
    local size=G.get(state.scene,'size'); local scale=math.min(1,width/G.get(size,'w'),height/G.get(size,'h'))
    Edges.draw(imgui,imgui.GetBackgroundDrawList_Nil(),state.scene,0,0,scale)
    renderer.draw(state.scene,width,height)
    imgui.Render(); imgui.love.RenderDrawLists()
    if evidence and frames==4 then love.graphics.captureScreenshot(function(data) local f=assert(io.open(evidence..'/render.png','wb')); f:write(data:encode('png'):getString()); f:close() end) end
  end
  function love.quit() imgui.love.Shutdown() end
  function love.textinput(t) imgui.love.TextInput(t) end
  function love.keypressed(k) imgui.love.KeyPressed(k) end
  function love.keyreleased(k) imgui.love.KeyReleased(k) end
end
return M
