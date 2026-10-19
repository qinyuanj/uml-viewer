-- Source presentation and byte-boundary selections; independent of the UI.
local M={}
local function char_size(text,i)
  local b=text:byte(i); return b<128 and 1 or b<224 and 2 or b<240 and 3 or 4
end
-- Selection stays in original source bytes; only paint/hit coordinates expand tabs.
function M.display(text)
  local out,to_source,to_display={}, {}, {}; local i,d=1,1
  while i<=#text do
    local n=char_size(text,i); local tab=text:sub(i,i)=='\t'
    local painted=tab and '  ' or text:sub(i,i+n-1)
    to_display[i]=d
    for j=0,#painted-1 do to_source[d+j]=i end
    out[#out+1]=painted; i=i+n; d=d+#painted
    to_source[d]=i; to_display[i]=d
  end
  to_source[d]=i; to_display[i]=d
  return {text=table.concat(out),to_source=to_source,to_display=to_display}
end
local keywords={}
for w in ('and break do else elseif end false for function if in local nil not or repeat return then true until while class def elif except finally from import lambda None pass raise try with yield async await as const export extends interface let new public private static this throw var void fn impl match mod mut pub self struct trait use enum where loop move ref unsafe'):gmatch('%S+') do keywords[w]=true end
function M.tokens(text,lang)
  local out,i={},1
  local function emit(last,kind) out[#out+1]={text=text:sub(i,last),kind=kind}; i=last+1 end
  while i<=#text do
    local rest=text:sub(i); local ch=text:sub(i,i)
    if (lang=='lua' and rest:sub(1,2)=='--') or (lang=='python' and ch=='#') or ((lang=='rust' or lang=='typescript') and rest:sub(1,2)=='//') then emit(#text,'comment')
    elseif ch=='"' or ch=="'" or ch=='`' then
      local j=i+1
      while j<=#text do if text:sub(j,j)=='\\' then j=j+2 elseif text:sub(j,j)==ch then j=j+1; break else j=j+1 end end
      emit(math.min(#text,j-1),'string')
    else
      local first=text:byte(i)
      if ch:match('[%a_]') or first>=128 then
        local j=i
        while j<=#text do
          local b=text:byte(j); local c=text:sub(j,j)
          if not c:match('[%w_]') and b<128 then break end
          j=j+char_size(text,j)
        end
        local word=text:sub(i,j-1); emit(j-1,keywords[word] and 'keyword' or 'ink')
      else emit(i,'ink') end
    end
  end
  return out
end
function M.ordered(a,b)
  if a.row>b.row or (a.row==b.row and a.col>b.col) then return b,a end
  return a,b
end
function M.selected(rows,a,b)
  if not a or not b then return '' end
  a,b=M.ordered(a,b); local out={}
  for i=a.row,b.row do
    local text=rows[i].text
    out[#out+1]=text:sub(i==a.row and a.col or 1,i==b.row and b.col-1 or #text)
  end
  return table.concat(out,'\n')
end
return M
