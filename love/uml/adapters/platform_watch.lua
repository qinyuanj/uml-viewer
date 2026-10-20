-- File timestamp boundary: an opaque stamp compared for equality, with
-- sub-second resolution so two edits inside one second still reload.
local M=require('uml.adapters.posix_watch')
return M
