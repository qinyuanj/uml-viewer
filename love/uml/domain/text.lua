-- Pure argv/text quoting shared by the Herdr boundary. A pane command string is
-- parsed by the pane's own shell, so every argument is quoted for that shell.
local M={}

function M.shell_word(text) return "'"..tostring(text):gsub("'","'\"'\"'").."'" end
return M
