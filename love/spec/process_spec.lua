local Process=require('uml.adapters.process')

describe('external process boundary',function()
  it('preserves actual stdout, stderr and exit status',function()
    local result=Process.run({'pwsh.exe','-NoProfile','-NonInteractive','-Command',
      "[Console]::Out.Write('out value'); [Console]::Error.Write('err value'); exit 23"},10000)
    assert.are.equal(23,result.exit)
    assert.are.equal('out value',result.out)
    assert.are.equal('err value',result.err)
    assert.is_false(result.timeout)
  end)
  it('reports a timeout and terminates the child',function()
    local result=Process.run({'pwsh.exe','-NoProfile','-NonInteractive','-Command','Start-Sleep -Seconds 10'},200)
    assert.is_true(result.timeout)
    assert.is_nil(result.exit)
    assert.are.equal('process timed out',result.err)
  end)
  it('removes only the named variables from the real child environment',function()
    local result=Process.run({'pwsh.exe','-NoProfile','-NonInteractive','-Command',
      "[Console]::Write('U=' + [string]$env:UML_PROCESS_TEST_VALUE)"},10000,{'UML_PROCESS_TEST_VALUE'})
    assert.are.equal(0,result.exit)
    assert.are.equal('U=',result.out)
  end)
  it('reads exact process creation ticks and treats only a positively absent pid as nil',function()
    local owner=Process.spawn_detached('pwsh.exe',{'-NoProfile','-NonInteractive','-Command','Start-Sleep -Seconds 2'},nil,nil)
    assert.is_truthy(tostring(owner.pid):match('^[1-9]%d*$'))
    local observed=Process.owner(owner.pid)
    assert.are.equal(owner.pid,observed.pid)
    assert.are.equal(owner.start,observed.start)
    assert.are.equal('pwsh',observed.name)
    local waited=Process.run({'pwsh.exe','-NoProfile','-NonInteractive','-Command',
      '[Diagnostics.Process]::GetProcessById('..owner.pid..').WaitForExit()'},10000)
    assert.are.equal(0,waited.exit)
    assert.is_nil(Process.owner(owner.pid))
  end)
  it('treats a terminated process held open by another handle as absent',function()
    -- Windows keeps a terminated process object openable while any handle to it
    -- remains. Liveness, not openability, decides whether a controller runs.
    local ping=(os.getenv('SystemRoot') or 'C:\\Windows')..'\\System32\\ping.exe'
    local pidfile=(os.getenv('TEMP') or '.')..'/uml-zombie-'..tostring(os.time())
      ..'-'..tostring(math.floor(os.clock()*1000))..'.pid'
    os.remove(pidfile)
    local script='$p=Start-Process -FilePath '..Process.ps_word(ping)
      ..' -ArgumentList '..Process.ps_word('-n')..','..Process.ps_word('1')..','
      ..Process.ps_word('127.0.0.1')..' -PassThru;'
      ..'$p.WaitForExit();'
      ..'[IO.File]::WriteAllText('..Process.ps_word(pidfile)..',[string]$p.Id);'
      ..'Start-Sleep -Seconds 3'
    local holder=Process.spawn_detached('pwsh.exe',
      {'-NoProfile','-NonInteractive','-Command',script},nil,nil)
    local deadline=os.clock()+15
    while os.clock()<deadline do
      local file=io.open(pidfile,'rb')
      if file then
        local text=file:read('*a'); file:close()
        local pid=tonumber((text or ''):match('^%s*(%d+)%s*$'))
        if pid then
          assert.is_nil(Process.owner_native(pid))
          assert.is_nil(Process.owner(pid))
          os.remove(pidfile)
          Process.run({'pwsh.exe','-NoProfile','-NonInteractive','-Command',
            '[Diagnostics.Process]::GetProcessById('..holder.pid..').WaitForExit()'},15000)
          return
        end
      end
    end
    error('zombie holder did not report its child pid')
  end)
  it('quotes Windows command-line arguments literally',function()
    assert.are.equal('plain',Process.win_quote('plain'))
    assert.are.equal('""',Process.win_quote(''))
    assert.are.equal('"a b"',Process.win_quote('a b'))
    assert.are.equal('"say \\"hi\\""',Process.win_quote('say "hi"'))
  end)
end)
