-- Native harness configuration: the same fixed spec paths as the pure suite's
-- runner, but without the pure guard, which forbids the real process access
-- these specs exist to verify. Nothing else about discovery changes.
return {
  default = {
    lpath = 'love/?.lua;love/?/init.lua',
    ROOT = { 'scripts', 'love/spec' },
    pattern = '(_spec)%.lua$',
  },
}
