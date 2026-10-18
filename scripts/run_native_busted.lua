-- Explicit acceptance harness; fixed spec path and no pure/mutation switch.
package.path='love/?.lua;love/?/init.lua;'..package.path
arg={'--ignore-lua','--helper=scripts/native_test_guard.lua','scripts/native_process_spec.lua','love/spec/file_spec.lua'}
require('busted.runner')({standalone=false})
