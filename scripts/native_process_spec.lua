-- Explicit native acceptance only: never discovered under love/spec or mutation.
local Process=require('uml.adapters.process')
describe('native bounded process fixtures',function()
  it('preserves stdout, stderr and exit status',function()
    local r=Process.run({'pwsh.exe','-NoProfile','-NonInteractive','-Command',"[Console]::Out.Write('out value'); [Console]::Error.Write('err value'); exit 23"},10000)
    assert.equals(23,r.exit); assert.equals('out value',r.out); assert.equals('err value',r.err); assert.is_false(r.timeout)
  end)
  it('reports a timeout for the bounded fixture',function()
    local r=Process.run({'pwsh.exe','-NoProfile','-NonInteractive','-Command','Start-Sleep -Seconds 10'},200)
    assert.is_true(r.timeout); assert.is_nil(r.exit); assert.equals('process timed out',r.err)
  end)
  it('removes only named environment variables',function()
    local r=Process.run({'pwsh.exe','-NoProfile','-NonInteractive','-Command',"[Console]::Write('U=' + [string]$env:UML_PROCESS_TEST_VALUE)"},10000,{'UML_PROCESS_TEST_VALUE'})
    assert.equals(0,r.exit); assert.equals('U=',r.out)
  end)
  it('reads exact creation identity and waits for normal exit',function()
    local owner=Process.spawn_detached('pwsh.exe',{'-NoProfile','-NonInteractive','-Command','Start-Sleep -Seconds 2'})
    assert.is_truthy(tostring(owner.pid):match('^[1-9]%d*$'))
    local observed=Process.owner(owner.pid)
    assert.equals(owner.pid,observed.pid); assert.equals(owner.start,observed.start); assert.equals('pwsh',observed.name)
    local r=Process.run({'pwsh.exe','-NoProfile','-NonInteractive','-Command','[Diagnostics.Process]::GetProcessById('..owner.pid..').WaitForExit()'},10000)
    assert.equals(0,r.exit); assert.is_nil(Process.owner(owner.pid))
  end)
  it('returns without waiting for a detached child to release stdio',function()
    local Clock=require('uml.adapters.clock')
    local started=Clock.monotonic_ms()
    local owner=Process.spawn_detached('pwsh.exe',{'-NoProfile','-NonInteractive','-Command','Start-Sleep -Seconds 5'})
    local elapsed=Clock.monotonic_ms()-started
    local r=Process.run({'pwsh.exe','-NoProfile','-NonInteractive','-Command',
      '[Diagnostics.Process]::GetProcessById('..owner.pid..').WaitForExit()'},15000)
    assert.equals(0,r.exit)
    assert.is_true(elapsed<3000,'spawn_detached waited '..tostring(elapsed)..'ms for the detached child')
  end)
  it('recognizes a terminated process even while its object remains open',function()
    local path=os.tmpname(); os.remove(path)
    local ping=(os.getenv('SystemRoot') or 'C:\\Windows')..'\\System32\\ping.exe'
    local script='$p=Start-Process -FilePath '..Process.ps_word(ping)..' -ArgumentList "-n","1","127.0.0.1" -PassThru;'
      ..'$p.WaitForExit();[IO.File]::WriteAllText('..Process.ps_word(path)..',[string]$p.Id);Start-Sleep -Seconds 3'
    local holder=Process.spawn_detached('pwsh.exe',{'-NoProfile','-NonInteractive','-Command',script})
    local ok,err=pcall(function()
      local deadline=os.clock()+15
      while os.clock()<deadline do
        local f=io.open(path,'rb')
        if f then
          local pid=tonumber(f:read('*a')); f:close()
          if pid then assert.is_nil(Process.owner_native(pid)); assert.is_nil(Process.owner(pid)); return end
        end
      end
      error('bounded identity fixture did not report its pid')
    end)
    Process.run({'pwsh.exe','-NoProfile','-NonInteractive','-Command','[Diagnostics.Process]::GetProcessById('..holder.pid..').WaitForExit()'},15000)
    os.remove(path); assert.is_true(ok,err)
  end)
end)
