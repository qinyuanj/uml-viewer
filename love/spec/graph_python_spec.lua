local E=require('uml.edn')
local function get(m,k) return E.get(m,E.keyword(k)) end
local function scanner() return require('uml.python_language.graph_python') end
local function eq(expected,actual) assert.is_true(E.equal(E.parse(expected),actual),E.encode(actual)) end
describe('Python graph snapshots',function()
  it('filters the JVM discovery names and directory components while retaining root and near matches',function()
    for _,p in ipairs{'x.py','test_.py','_test.py','test/x.py','target/x.py','app/test/x.py'} do assert.is_true(scanner().accepts(p),p) end
    for _,p in ipairs{'x.PY','x.txt','test_a.py','foo_test.py','foo_tests.py','conftest.py','.hidden/a.py','app/.hidden/a.py','__pycache__/a.py','venv/a.py','site-packages/a.py','dist/a.py','build/a.py','node_modules/a.py','tests/a.py','__pypackages__/a.py'} do assert.is_false(scanner().accepts(p),p) end
    assert.is_true(scanner().accepts_directory('')); assert.is_false(scanner().accepts_directory('app/tests'))
  end)
  it('resolves dotted imports, aliases, relative children, star fallback and overclimbs without self edges',function()
    local s=scanner().scan('C:/p/src/app',E.parse('{:prefix "app"}'),{
      {path='C:/p/src/app/__init__.py',source='from . import model, missing\nfrom app import *'},
      {path='C:/p/src/app/extra.py',source='import app.model\nfrom . import model\nfrom app.model import Animal as A\nfrom ext import *\nfrom .... import missing\nclass Dog(app.model.Animal, model.Animal, A):\n pass'},
      {path='C:/p/src/app/model.py',source='class Animal:\n pass'},
    },'C:/p')
    eq('[{:from :app :to :model :kind :dependency} {:from :extra :to :model :kind :inheritance} {:from :extra :to :model :kind :dependency} {:from :extra :to :ext :kind :dependency}]',get(s,'edges'))
    assert.equals('app.model',get(get(s,'classes')[3],'ns')); eq(':ext',get(get(s,'classes')[4],'id'))
  end)
  it('marks only all-marker class modules as interfaces and resolves project marker bases',function()
    local s=scanner().scan('C:/p',E.parse('{:prefix "app" :ns-prefix ""}'),{
      {path='C:/p/marker.py',source='class Protocol:\n pass'},
      {path='C:/p/pure.py',source='from marker import Protocol\nclass P(Protocol):\n pass\nclass A(abc.ABC, abc.ABCMeta):\n pass'},
      {path='C:/p/mixed.py',source='class P(Protocol):\n pass\nclass Plain:\n pass'},
      {path='C:/p/function.py',source='class P(Protocol):\n pass\ndef run(): pass'},
    },'C:/p')
    local cs=get(s,'classes'); eq(':interface',get(cs[4],'stereotype')); assert.is_nil(get(cs[1],'stereotype')); assert.is_nil(get(cs[3],'stereotype'))
    eq('[{:from :pure :to :marker :kind :implements} {:from :pure :to :marker :kind :dependency}]',get(s,'edges'))
  end)
  it('generates fixture IR semantically equal to the independent JVM golden',function()
    local lfs=require('lfs'); local IR=require('uml.application.ir_generator')
    local root=lfs.currentdir():gsub('\\','/')..'/python-fixture'; local files={}
    local function read(path) local f=assert(io.open(path,'rb')); local s=f:read('*a'); f:close(); return s end
    local function visit(dir) for n in lfs.dir(dir) do if n~='.' and n~='..' then local p=dir..'/'..n; if lfs.attributes(p,'mode')=='directory' then visit(p) else files[#files+1]={path=p,source=read(p):gsub('\r\n','\n')} end end end end
    visit(root..'/src') -- The oracle was captured from Git's LF fixture; do not change scanner input handling.
    local p=E.parse(read(root..'/policy.edn'))
    local doc=IR.document(function(_,opts) return scanner().scan(root..'/src',opts,files,root) end,p)
    E.assoc(doc,E.keyword('policy-file'),'policy.edn')
    assert.is_true(E.equal(doc,E.parse(read('fixtures/jvm-goldens/python.ir.edn'))),E.encode(doc))
  end)
  it('preserves package identity, inheritance, protocol markers and simple return implementations',function()
    local s=scanner().scan('C:/p/src',E.parse('{:prefix "app"}'),{
      {path='C:/p/src/app/__init__.py',source='from .model import Animal, walk'},
      {path='C:/p/src/app/model.py',source='class Animal:\n    pass\ndef walk(x):\n    pass'},
      {path='C:/p/src/app/dogs/__init__.py',source='from ..model import Animal\nfrom .kennel import Kennel\nclass Pack(Animal):\n    pass'},
      {path='C:/p/src/app/dogs/kennel.py',source='from ..model import Animal\nimport os.path\nclass Kennel(Animal):\n    pass'},
      {path='C:/p/src/app/fs.py',source='from typing import Protocol\nclass Fs(Protocol):\n    pass'},
      {path='C:/p/src/app/service.py',source='from .model import Animal\ndef load() -> Animal:\n    pass'},
      {path='C:/p/src/tests/noise.py',source='import nope'},
      {path='C:/p/src/app/foo_test.py',source='import nope'},
    },'C:/p')
    eq('[{:from :app :to :model :kind :dependency} {:from :dogs :to :model :kind :inheritance} {:from :dogs :to :model :kind :dependency} {:from :dogs :to :dogs.kennel :kind :dependency} {:from :dogs.kennel :to :model :kind :inheritance} {:from :dogs.kennel :to :model :kind :dependency} {:from :dogs.kennel :to :os.path :kind :dependency} {:from :fs :to :typing :kind :dependency} {:from :service :to :model :kind :implements} {:from :service :to :model :kind :dependency}]',get(s,'edges'))
    local classes=get(s,'classes'); assert.equals(8,#classes)
    assert.equals('app',get(classes[1],'ns')); assert.equals('src/app/__init__.py',get(classes[1],'file'))
    eq(':interface',get(classes[4],'stereotype')); assert.equals('Model',get(classes[5],'name'))
    eq('[{:name "Animal" :text "Animal"} {:name "walk" :text "walk"}]',get(classes[5],'ops'))
  end)
end)
describe('Python module surface',function()
  it('masks escaped raw triple and nested f-strings without swallowing subsequent imports',function()
    local source=[==[s = "say \"import hidden\""
t = r'raw\nimport hidden'
q = r'it\'import hidden'
u = f"a {'import hidden'}"
v = rf"{{escaped}} {f'{"nested"}'}"
w = F"{'import hidden'}"
x = u"import hidden"
y = b'import hidden'
z = R'''import hidden'''
def load(xs=[1], row={}):
    return xs
import os]==]
    local s=scanner().read_module(source)
    eq('[{:kind :import :modules [{:name "os" :bind "os"}]}]',get(s,'imports'))
    eq('[{:name "load" :kind :function}]',get(s,'defs'))
  end)
  it('retains the JVM exact backslash LF continuation rule with CRLF input',function()
    eq('[{:kind :import :modules [{:name "os" :bind "os"}]}]',get(scanner().read_module('from app.model \\\r\n import Animal\r\nimport os\r\n'),'imports'))
  end)
  it('stops unclosed literals at EOF',function()
    for _,source in ipairs{'s = "import hidden','s = f"{\'nested\'','s = """\nclass Hidden:\n pass'} do
      eq('{:imports [] :defs []}',scanner().read_module(source))
    end
  end)
  it('retains semicolon and name-list quirks, ignores complex annotations and private definitions',function()
    local s=scanner().read_module('x=1; import first\nimport a; import b\nfrom . import *\nfrom package import X as Y, child.mod\nasync def run() -> Thing:\n pass\nclass Public(Base, pkg.Other, Generic[X], metaclass=ABCMeta):\n pass\n def nested(): pass\ndef _private(): pass\ndef complex() -> pkg.Thing: pass')
    eq('[{:kind :import :modules [{:name "first" :bind "first"}]} {:kind :import :modules []} {:kind :from :dots 1 :names []} {:kind :from :dots 0 :module "package" :names [{:name "X" :bind "Y"} {:name "child.mod" :bind "child"}]}]',get(s,'imports'))
    eq('[{:name "run" :kind :function :return "Thing"} {:name "Public" :kind :class :bases ["Base" "pkg.Other"]} {:name "complex" :kind :function}]',get(s,'defs'))
  end)
  it('reads continued and local imports, public module definitions, bases and simple returns',function()
    local s=scanner().read_module([=["""
import nope
"""
s = f"say {'import nope'}"
# import nope
from app.model \
    import Animal
from app.model import (
    Walk,
)
import os.path
def load() -> Animal:
    import json
    return Animal()
def all_animals() -> list[Animal]:
    return []
def _hidden():
    pass
class Dog(Animal):
    def speak(self):
        pass
]=])
    eq('[{:kind :from :dots 0 :module "app.model" :names [{:name "Animal" :bind "Animal"}]} {:kind :from :dots 0 :module "app.model" :names [{:name "Walk" :bind "Walk"}]} {:kind :import :modules [{:name "os.path" :bind "os"}]} {:kind :import :modules [{:name "json" :bind "json"}]}]',get(s,'imports'))
    eq('[{:name "load" :kind :function :return "Animal"} {:name "all_animals" :kind :function} {:name "Dog" :kind :class :bases ["Animal"]}]',get(s,'defs'))
  end)
end)
