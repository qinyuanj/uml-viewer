-- Run from repository root; interpreter receives Busted arguments after this file.
package.path = 'love/?.lua;love/?/init.lua;' .. package.path
require('busted.runner')({ standalone = false })
