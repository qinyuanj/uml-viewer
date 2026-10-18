-- Actual LOVE/cimgui probe in an owned process. No existing viewer is touched.
return function(frame)
  if not _G.uml68_window then
    local im=require('cimgui')
    local window=require('uml.adapters.source_window').new(im,{source=require('uml.main.source')})
    local path=os.getenv('UML68_SOURCE_FILE')
    assert(window.open({lang='lua',file=path,name='M.target'}))
    assert(window.view().line==82)
    _G.uml68_window=window
    local draws=0
    love.draw=function()
      draws=draws+1
      love.graphics.clear(.09,.10,.12,1)
      window.draw(); im.Render(); im.love.RenderDrawLists()
      if draws==6 then
        love.graphics.captureScreenshot(function(data)
          local f=assert(io.open(os.getenv('UML68_EVIDENCE')..'/source-window.png','wb')); f:write(data:encode('png'):getString()); f:close()
        end)
        local f=assert(io.open(os.getenv('UML68_EVIDENCE')..'/source-window.txt','wb'))
        f:write(window.view().file..'\nline='..window.view().line..'\nrows='..#window.view().rows..'\n'); f:close()
      end
    end
  end
end
