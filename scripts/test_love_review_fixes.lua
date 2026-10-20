-- Owned native process: process-local input and clipboard callbacks only.
local ffi=require('ffi')
local failed
local function check(ok,message) if not ok then failed=failed or message end end
local function save(name,text)
  local f=assert(io.open(os.getenv('UML78_EVIDENCE')..'/'..name,'wb')); f:write(text); f:close()
end
return function(frame,state,ui)
  if os.getenv('UML78_THROW')=='1' then error('intentional first-frame probe failure') end
  local im=ui.imgui; local io=im.GetIO()
  if frame==1 then
    -- Match LOVE's modifier query to this process's injected ImGui input;
    -- never synthesize a desktop/global key press.
    local is_down=love.keyboard.isDown
    love.keyboard.isDown=function(...)
      for _,key in ipairs{...} do if key=='lctrl' or key=='rctrl' then return io.KeyCtrl end end
      return is_down(...)
    end
    local clipboard=''
    _G.uml78_clipboard_get=ffi.cast('const char*(*)(void*)',function() return clipboard end)
    _G.uml78_clipboard_set=ffi.cast('void(*)(void*, const char*)',function(_,text) clipboard=ffi.string(text) end)
    io.GetClipboardTextFn=_G.uml78_clipboard_get; io.SetClipboardTextFn=_G.uml78_clipboard_set
    local window=require('uml.adapters.source_window').new(im,{ui=.45,source={member_source=function(ident) return {title=ident.name,file=ident.name,body=ident.name,line=nil} end}})
    _G.uml78_windows=window
    assert(window.open{name='FIRST'}); assert(window.open{name='SECOND'})
    local original=love.draw
    local external=ffi.new('char[64]','OTHER WIDGET')
    love.draw=function()
      -- Stable positions avoid depending on the user's persisted ImGui layout.
      im.SetNextWindowPos(im.ImVec2_Float(450,150),im.ImGuiCond_Always)
      im.SetNextWindowSize(im.ImVec2_Float(420,260),im.ImGuiCond_Always)
      window.windows()[2].draw()
      im.SetNextWindowPos(im.ImVec2_Float(10,10),im.ImGuiCond_Always)
      im.SetNextWindowSize(im.ImVec2_Float(420,260),im.ImGuiCond_Always)
      window.windows()[1].draw()
      im.SetNextWindowPos(im.ImVec2_Float(10,300),im.ImGuiCond_Always)
      im.SetNextWindowSize(im.ImVec2_Float(420,160),im.ImGuiCond_Always)
      im.Begin('Other widget'); im.InputText('##external-copy',external,64); im.End()
      original()
    end
  end
  local window=_G.uml78_windows
  if frame>1 then
    local a,b=window.windows()[1].viewport(),window.windows()[2].viewport()
    local f=assert(_G.io.open(os.getenv('UML78_EVIDENCE')..'/trace.txt','ab'))
    f:write(frame..' first='..tostring(a and a.selection)..' second='..tostring(b and b.selection)..' a='..tostring(a and a.lines[1].x)..','..tostring(a and a.lines[1].y)..' b='..tostring(b and b.lines[1].x)..','..tostring(b and b.lines[1].y)..'\n'); f:close()
  end
  local function point(index,last)
    local p=assert(window.windows()[index].viewport()).lines[1]
    io:AddMousePosEvent(p.x+(last and im.CalcTextSize(index==1 and 'FIRST' or 'SECOND').x or 0),p.y+p.h/2)
  end
  if frame==3 then point(1,false); io:AddMouseButtonEvent(0,true)
  elseif frame==5 then point(1,true)
  elseif frame==7 then io:AddMouseButtonEvent(0,false)
  elseif frame==9 then
    check(window.windows()[1].viewport().selection=='FIRST','first window must retain FIRST selection')
    point(2,false); io:AddMouseButtonEvent(0,true)
  elseif frame==11 then point(2,true)
  elseif frame==13 then io:AddMouseButtonEvent(0,false)
  elseif frame==15 then
    check(window.windows()[1].viewport().selection=='FIRST','second drag must preserve first selection, got '..tostring(window.windows()[1].viewport().selection))
    check(window.windows()[2].viewport().selection=='SECOND','second window must select SECOND')
    io:AddKeyEvent(im.ImGuiMod_Ctrl,true); io:AddKeyEvent(im.ImGuiKey_C,true)
  elseif frame==17 then
    check(ffi.string(im.GetClipboardText())=='SECOND','focused SECOND copy was overwritten: '..ffi.string(im.GetClipboardText()))
    io:AddKeyEvent(im.ImGuiKey_C,false); io:AddKeyEvent(im.ImGuiMod_Ctrl,false)
    io:AddMousePosEvent(80,340); io:AddMouseButtonEvent(0,true)
  elseif frame==19 then io:AddMouseButtonEvent(0,false)
  elseif frame==21 then io:AddKeyEvent(im.ImGuiMod_Ctrl,true); io:AddKeyEvent(im.ImGuiKey_A,true)
  elseif frame==23 then io:AddKeyEvent(im.ImGuiKey_A,false)
  elseif frame==25 then io:AddKeyEvent(im.ImGuiKey_C,true)
  elseif frame==27 then
    check(ffi.string(im.GetClipboardText())=='OTHER WIDGET','other widget copy overwritten: '..ffi.string(im.GetClipboardText()))
    io:AddKeyEvent(im.ImGuiKey_C,false); io:AddKeyEvent(im.ImGuiMod_Ctrl,false)
    if failed then error(failed) end
    save('clipboard-pass.txt','PASS two independent source selections; focused copy SECOND; other widget copy OTHER WIDGET\n')
    love.event.quit(0)
  end
end
