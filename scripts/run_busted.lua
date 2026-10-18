-- Run from repository root; interpreter receives Busted arguments after this file.
package.path = 'love/?.lua;love/?/init.lua;' .. package.path
require('scripts.pure_test_guard')
require('busted.runner')({ standalone = false })
