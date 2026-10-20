-- Memo for the selected class card. Building it costs two hierarchy compiles
-- (~90 ms on the example diagram), and both the sidebar summary and the detail
-- window ask for the same model every frame, so one entry is kept per document,
-- revision and target.
local E=require('uml.edn')
local Detail=require('uml.application.detail')
local M={}
function M.new()
  local model,rows,document,revision,cached_id
  return function(state,id)
    if id==nil then return nil end
    if model and document==state.document and revision==state.revision
        and cached_id~=nil and E.equal(cached_id,id) then
      return model,rows
    end
    model=Detail.for_state(state,id)
    rows=model and Detail.rows(model) or nil
    document=state.document; revision=state.revision; cached_id=id
    return model,rows
  end
end
return M
