-- Explicit POSIX native acceptance harness. Fixed spec paths, no pure/mutation
-- switch: these specs spawn real processes and read real /proc identities.
package.path='love/?.lua;love/?/init.lua;'..package.path
-- The repository .busted helper installs the pure guard, which forbids the very
-- process access these specs exist to verify, so the harness loads its own
-- configuration instead: same paths, no helper.
arg={'--ignore-lua','--config-file=scripts/native_busted_config.lua',
  'scripts/posix_process_spec.lua','love/spec/file_spec.lua'}
require('busted.runner')({standalone=false})
