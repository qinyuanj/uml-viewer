-- Process boundary. argv is passed literally, results are data, and a
-- creation identity is opaque text compared for exact equality. POSIX is the
-- only implementation (ADR 0007): Windows native is not a delivery platform.
local M=require('uml.adapters.posix_process')
return M
