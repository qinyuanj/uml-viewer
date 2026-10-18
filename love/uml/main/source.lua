local Source=require('uml.source')
Source.register('typescript',require('uml.typescript_language.source_typescript'))
Source.register('python',require('uml.python_language.source_python'))
Source.register('rust',require('uml.rust_language.source_rust'))
Source.register('lua',require('uml.lua_language.source_lua'))
return Source
