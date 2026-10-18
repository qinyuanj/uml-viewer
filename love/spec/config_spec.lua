-- Original config_spec: 31 examples. The 30 public quality behaviors are
-- consolidated into three scenarios. The private lerp equal-stops example is
-- retired: fixed public grading thresholds cannot create equal adjacent stops.
local E = require('uml.edn')
local C = require('uml.domain.config')
local function same(expected, actual) assert.is_true(E.equal(E.parse(expected), actual)) end

describe('quality in view state', function()
  it('pools functions and measured summaries, counting absent metrics as red gaps', function()
    local cases={
      {'[{:ops [{:crap 2} {:crap 2} {:crap 2}]} {:ops [{:crap 20}]}]','{:mu 6.5 :max 20 :sigma 7.8 :n 4}'},
      {'[{:crap {:mu 2 :sigma 0 :max 2 :n 3}} {:crap {:mu 20 :sigma 0 :max 20 :n 1}}]','{:mu 6.5 :max 20 :sigma 7.8 :n 4}'},
      {'[{:crap {:mu 2 :sigma 0 :max 2 :n 10}} {:ops [{:name "a"} {:name "b"}]}]','{:mu 5 :max 20 :sigma 6.7 :n 12}'},
      {'[{:crap {:mu 2 :sigma 0 :max 2 :n 1}} {}]','{:mu 11 :max 20 :sigma 9 :n 2}'},
      {'[{:crap {:mu 2 :sigma 0 :max 2 :n 10} :ops [{:crap 20}]}]','{:mu 2 :max 2 :sigma 0 :n 10}',true}
    }
    for _,r in ipairs(cases) do same(r[2], C.pool_crap(E.parse(r[1]),r[3])) end
    for _,r in ipairs{
      {'[{:killed 9 :survived 1} {:killed 1 :survived 1}]','{:killed 10 :survived 2}'},
      {'[{:killed 9 :survived 1} {:ops [{} {}]}]','{:killed 9 :survived 1 :mut-gap 2}'},
      {'[{:killed 9 :survived 1 :mut-gap 1} {:ops [{} {}]}]','{:killed 9 :survived 1 :mut-gap 3}'}
    } do same(r[2], C.pool_mutants(E.parse(r[1]))) end
    same('{:mu 3 :max 4 :sigma 1 :n 2}', C.summarize_scores(E.parse('[2 {:mu 4} {}]')))
    assert.is_nil(C.summarize_scores(E.parse('[]')))
    assert.is_nil(C.pool_crap(E.parse('[{:foreign true} {:shape :oval}]')))
    assert.is_nil(C.pool_mutants(E.parse('[{:foreign true} {:shape :oval}]')))
  end)
  it('grades measured quality continuously and missing quality red', function()
    same('{:red 0.8 :yellow 0.9 :green 1}', C.mutation_thresholds)
    assert.are.equal('red', C.mutation_band(nil).name)
    for _, r in ipairs{{0,'red'},{0.8,'red'},{0.81,'yellow'},{0.9,'yellow'},{0.91,'green'},{1,'green'}} do assert.are.equal(r[2], C.mutation_band(r[1]).name) end
    for _, r in ipairs{{0,10},{8,10},{10,7.75},{12,5.5},{20,1},{100,1}} do assert.are.equal(r[2], C.crap_grade(r[1])) end
    for _, r in ipairs{{0,1},{0.8,1},{0.9,5.5},{1,10}} do assert.are.equal(r[2], C.mutation_grade(r[1])) end
    assert.are.equal(1, C.crap_grade(nil)); assert.are.equal(1, C.mutation_grade(nil))
    assert.are.equal(3.25,C.crap_grade(16)); assert.is_true(math.abs(C.mutation_grade(0.95)-7.75)<1e-12)
    assert.are.equal(6,C.crap_risk(E.parse('{:mu 6}')))
    assert.are.equal(12, C.crap_risk(E.parse('{:mu 6 :sigma 6}')))
    assert.is_nil(C.crap_risk(E.parse('{}')))
    assert.are.equal(5.5, C.combined_grade(10,1)); assert.are.equal(10,C.combined_grade(10,nil)); assert.are.equal(1,C.combined_grade(nil,1)); assert.is_nil(C.combined_grade(nil,nil))
    for _, r in ipairs{{'{:killed 4 :survived 0}',1},{'{:killed 4}',1},{'{:survived 3}',0},{'{:killed 8 :survived 2}',0.8},{'{:killed 9 :survived 1 :mut-gap 2}',0.75}} do assert.are.equal(r[2],C.mutation_ratio(E.parse(r[1]))) end
    assert.is_nil(C.mutation_ratio(E.parse('{:killed 0 :survived 0}'))); assert.is_nil(C.mutation_ratio(E.parse('{}')))
  end)
  it('bands missing CRAP red and preserves the traffic-light boundaries', function()
    assert.are.equal('red', C.crap_band(nil).name)
    for _, row in ipairs{{0,'green'},{8,'green'},{8.1,'yellow'},{12,'yellow'},{12.1,'red'},{20,'red'},{100,'red'}} do
      assert.are.equal(row[2], C.crap_band(row[1]).name)
    end
    same('{:green 8 :yellow 12 :red 20}', C.crap_thresholds)
  end)
end)
