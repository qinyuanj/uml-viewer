package.path='love/?.lua;love/?/init.lua;'..package.path
local E=require('uml.edn')
local function read(path) local f=assert(io.open(path,'rb')); local text=f:read('*a'); f:close(); return E.parse(text) end
assert(E.equal(read(arg[1]),read(arg[2])),'IR semantic comparison failed: '..arg[1]..' vs '..arg[2])
print('IR semantic comparison passed')
