-- Explicit POSIX native acceptance harness. Fixed spec paths, no pure/mutation
-- switch: these specs spawn real processes and read real /proc identities.
package.path='love/?.lua;love/?/init.lua;'..package.path
arg={'--ignore-lua','scripts/posix_process_spec.lua','love/spec/file_spec.lua'}
require('busted.runner')({standalone=false})
