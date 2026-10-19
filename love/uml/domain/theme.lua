-- Single viewer theme: one dark palette (the 4e69e34 inks), screen-px line
-- widths, radii and the continuous quality ramps shared by the canvas, edges,
-- source window, sidebar and detail window. Colors are RGB triples; the cimgui
-- ABGR packing is pack(). Quality thresholds and the missing-means-red rule
-- stay in domain.config: a missing metric enters the ramps as grade 1.
local M={}
M.fonts={body=18,title=18,badge=9,level=11,marker=12,code=13}
M.radius={class=4,package=8,button=4,window=4,swatch=2}
-- Baseline screen-px widths at DPI scale 1: never multiplied by canvas zoom.
-- dpi() is the single entry point a later DPI ticket scales through.
M.widths={class=2.5,package=2,selected=3.5,edge=1.6,edge_hot=2.8,edge_violating=2.4,edge_violating_hot=3.2,band=2,rule=1,dot=1,dash=6,dash_gap=4,bar=3}
M.colors={
  canvas_bg={22,28,32},panel={26,36,40},sidebar={18,22,24},line={42,61,54},
  ink={236,236,228},white={255,255,255},muted={157,184,168},gold={232,196,72},
  good={95,181,138},violation={196,42,36},violation_hot={255,64,48},warn={224,122,74},
  band={red={196,42,36},yellow={232,196,72},green={95,181,138}},
  edge={157,184,168},edge_hot={232,196,72},edge_violating={196,42,36},edge_violating_hot={255,64,48},
  indicator={157,184,168},
  source_bg={248,250,252},source_ink={17,24,39},source_header_bg={229,231,235},
  source_gutter_bg={238,242,247},source_gutter={107,114,128},source_gutter_rule={209,213,219},
  source_highlight={253,230,138},source_highlight_gutter={252,211,77},source_selection={191,219,254},
  source_comment={107,114,128},source_string={180,83,9},source_keyword={29,78,216},
}
-- Alphas (0-255) for washes and the lighter namespace frame.
M.alpha={package_fill=80,package_stroke=170,sidebar=255,row_wash=48,stripe=255,stripe_odd=0}
function M.pack(c,a) return (a or 255)*16777216+(c[3] or 0)*65536+(c[2] or 0)*256+(c[1] or 0) end
function M.band_color(band) return M.pack(M.colors.band[band] or M.colors.band.red) end
local function mix(a,b,t) local out={}; for i=1,3 do out[i]=math.floor(a[i]+t*(b[i]-a[i])+.5) end; return out end
local function ramp(g,none,red,mid,green)
  if g==nil then return none elseif g<=5 then return mix(red,mid,g/5) end
  return mix(mid,green,math.min(1,(g-5)/5))
end
-- Continuous 0-10 grade ramps (draw.clj fill-for / stroke-for).
function M.fill(g) return ramp(g,{36,52,48},{74,40,24},{61,58,24},{30,74,56}) end
function M.stroke(g) return ramp(g,{90,110,100},{224,122,74},{212,192,90},{95,181,138}) end
-- Line coverage ink: >=80% good, >=50% gold, else warn; unknown muted.
function M.coverage(p) return p==nil and M.colors.muted or p>=.8 and M.colors.good or p>=.5 and M.colors.gold or M.colors.warn end
-- One DPI conversion entry point; later scaling multiplies here, never inline.
function M.dpi(scale)
  scale=scale or 1; local out={}
  for k,v in pairs(M.widths) do out[k]=v*scale end
  return out
end
return M
