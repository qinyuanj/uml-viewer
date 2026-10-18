-- Isolated native Windows feasibility probe; never uses the viewer mailbox.
local ffi = require('ffi')
local dependencies = assert(os.getenv('UML52_DEPENDENCIES')):gsub('\\', '/')
local evidence = assert(os.getenv('UML52_EVIDENCE')):gsub('\\', '/')
package.path = dependencies .. '/?.lua;' .. dependencies .. '/?/init.lua;' .. package.path
package.cpath = dependencies .. '/?.dll;' .. package.cpath
local imgui = require('cimgui')
ffi.cdef[[
void* FindWindowA(const char*, const char*);
void* GetForegroundWindow(void);
int SetForegroundWindow(void*);
void keybd_event(unsigned char, unsigned char, unsigned long, uintptr_t);
void* LoadKeyboardLayoutA(const char*, unsigned int);
void* ActivateKeyboardLayout(void*, unsigned int);
void* GetKeyboardLayout(unsigned long);
]]
local user32 = ffi.load('user32')
local buffer = ffi.new('char[256]')
local frame, elapsed, stage = 0, 0, 0
local title = 'UML ticket 52 isolated IME probe'
local log = assert(io.open(evidence .. '/events.txt', 'w'))
local hwnd, oldLayout, font
local injection, composition, committed, submitted, captured = false, false, false, false, false
local function record(s) log:write(s .. '\n'); log:flush() end
local function screenshot(name)
    love.graphics.captureScreenshot(function(data)
        local bytes = data:encode('png')
        local f = assert(io.open(evidence .. '/' .. name .. '.png', 'wb'))
        f:write(bytes:getString()); f:close()
    end)
end
local function key(vk)
    assert(user32.GetForegroundWindow() == hwnd, 'Probe lost foreground; refusing keyboard input')
    user32.keybd_event(vk, 0, 0, 0)
    user32.keybd_event(vk, 0, 2, 0)
end
function love.load()
    love.window.setTitle(title)
    love.window.setMode(800, 320)
    record('runtime=' .. jit.version .. ' LOVE=' .. love.getVersion())
    imgui.love.Init()
    local io = imgui.GetIO()
    font = io.Fonts:AddFontFromFileTTF('C:/Windows/Fonts/msyh.ttc', 24, nil, io.Fonts:GetGlyphRangesChineseSimplifiedCommon())
    assert(font ~= nil, 'CJK font did not load')
    imgui.love.BuildFontAtlas()
    for _, cp in ipairs({0x4E2D, 0x6587, 0x4F60, 0x597D}) do
        assert(font:FindGlyphNoFallback(cp) ~= nil, 'Missing non-fallback glyph ' .. cp)
        record('non-fallback-glyph=' .. cp)
    end
    io.FontDefault = font
    love.keyboard.setTextInput(true, 50, 100, 600, 30)
    record('text-input-enabled=' .. tostring(love.keyboard.hasTextInput()))
    hwnd = user32.FindWindowA(nil, title)
    assert(hwnd ~= nil, 'Own probe window missing')
    user32.SetForegroundWindow(hwnd)
    oldLayout = user32.GetKeyboardLayout(0)
end
function love.textinput(text)
    record('textinput=' .. text)
    imgui.love.TextInput(text)
    if stage >= 2 and text:find('[\128-\255]') then committed = true end
end
function love.textedited(text, start, length)
    record('composition=' .. text .. ' start=' .. start .. ' length=' .. length)
    if text ~= '' then composition = true end
end
function love.keypressed(k) imgui.love.KeyPressed(k) end
function love.keyreleased(k) imgui.love.KeyReleased(k) end
function love.update(dt)
    frame = frame + 1; elapsed = elapsed + dt
    imgui.love.Update(dt); imgui.NewFrame()
    if frame == 8 then love.event.push('textinput', '中文') end
    if frame == 15 then
        injection = ffi.string(buffer) == '中文'
        record('injection=' .. tostring(injection) .. ' buffer=' .. ffi.string(buffer))
        assert(injection, 'Injected UTF-8 did not reach ImGui input buffer')
        screenshot('cjk-render')
    end
    if elapsed > 1 and stage == 0 then
        ffi.fill(buffer, 256)
        imgui.SetKeyboardFocusHere()
        local layout = user32.LoadKeyboardLayoutA('00000804', 1)
        user32.ActivateKeyboardLayout(layout, 0)
        record('requested-layout=' .. tostring(layout) .. ' actual=' .. tostring(user32.GetKeyboardLayout(0)))
        stage = 1
    elseif elapsed > 2 and stage == 1 then
        stage = 2
        for _, vk in ipairs({0x4E,0x49,0x48,0x41,0x4F}) do key(vk) end
        record('native-virtual-keys=NIHAO')
    elseif elapsed > 4 and stage == 2 then
        screenshot('ime-before-confirmation')
        record('composition-observed=' .. tostring(composition))
        key(0x20); stage = 3
        record('native-candidate-confirm=SPACE')
    elseif elapsed > 5 and stage == 3 then
        record('after-confirm-buffer=' .. ffi.string(buffer))
        key(0x0D); stage = 4
    elseif elapsed > 6 and stage == 4 then
        screenshot('ime-after-submission')
        record('final-buffer=' .. ffi.string(buffer))
        local f = assert(io.open(evidence .. '/result.txt', 'w'))
        f:write('glyphs=pass\ninjection=' .. tostring(injection) .. '\ncomposition=' .. tostring(composition) .. '\ncommit=' .. tostring(committed) .. '\nsubmission=' .. tostring(submitted) .. '\nreal-ime=unverified\n')
        f:close()
        record('HARD GATE: candidate UI and complete real IME acceptance require verified evidence; no pass claimed.')
        user32.ActivateKeyboardLayout(oldLayout, 0)
        stage = 5
    elseif elapsed > 7 then love.event.quit(0) end
end
function love.draw()
    imgui.SetNextWindowSize(imgui.ImVec2_Float(760, 260))
    imgui.Begin('CJK / IME feasibility')
    imgui.TextUnformatted('中文 你好')
    if frame < 5 then imgui.SetKeyboardFocusHere() end
    if imgui.InputText('Proposal', buffer, 256, imgui.ImGuiInputTextFlags_EnterReturnsTrue) then
        submitted = true; record('input-submitted=' .. ffi.string(buffer))
    end
    imgui.End()
    imgui.Render(); imgui.love.RenderDrawLists()
end
function love.quit()
    if oldLayout then user32.ActivateKeyboardLayout(oldLayout, 0) end
    log:close(); imgui.love.Shutdown()
end
function love.errorhandler(message)
    record('ERROR=' .. tostring(message))
    local f = io.open(evidence .. '/failure.txt', 'w')
    if f then f:write(tostring(message)); f:close() end
    return function() return 1 end
end
