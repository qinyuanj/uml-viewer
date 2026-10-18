-- LOVE callback installation; owns no companion, mailbox or existing viewer resources.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local IR=require('uml.domain.ir')
local D=require('uml.application.document')
local Paths=require('uml.adapters.windows_path')
local Watch=require('uml.adapters.file_watch')
local Boxes=require('uml.adapters.class_boxes')
local M={}
local function read(path) local f=assert(io.open(path,'rb')); local s=f:read('*a'); f:close(); return s end
function M.run(args)
  local path=args[1] or 'examples/uml-viewer.edn'
  assert(path:sub(1,1)~='-','usage: uml --runtime love [diagram.edn]')
  path=Paths.canonical(path)
  local dependencies=os.getenv('UML_CIMGUI_DIRECTORY')
  if dependencies then dependencies=dependencies:gsub('\\','/'); package.path=dependencies..'/?.lua;'..dependencies..'/?/init.lua;'..package.path; package.cpath=dependencies..'/?.dll;'..package.cpath end
  local imgui=require('cimgui')
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
    state=D.new(IR.read_document(read(path)),Watch.mtime(path)); publish()
  end
  function love.update(dt)
    elapsed=elapsed+dt; frames=frames+1
    local ok,stamp=pcall(Watch.mtime,path)
    if ok and stamp~=state.mtime then
      local loaded,text=pcall(read,path)
      if loaded then local next_state,changed,err=D.reload(state,text,stamp); state=next_state; if changed then publish() elseif err then io.stderr:write('IR reload rejected: '..err..'\n') end end
    end
    imgui.love.Update(dt); imgui.NewFrame()
    if test_seconds and elapsed>=test_seconds then love.event.quit(0) end
  end
  function love.draw()
    love.graphics.clear(.09,.10,.12,1)
    renderer.draw(state.scene,love.graphics.getWidth(),love.graphics.getHeight())
    imgui.Render(); imgui.love.RenderDrawLists()
    if evidence and frames==4 then love.graphics.captureScreenshot(function(data) local f=assert(io.open(evidence..'/render.png','wb')); f:write(data:encode('png'):getString()); f:close() end) end
  end
  function love.quit() imgui.love.Shutdown() end
  function love.textinput(t) imgui.love.TextInput(t) end
  function love.keypressed(k) imgui.love.KeyPressed(k) end
  function love.keyreleased(k) imgui.love.KeyReleased(k) end
end
return M
