local Source=require('uml.source')
return Source.file_source({
  'export%s+default%s+async%s+function%s+@',
  'export%s+async%s+function%s+@',
  'export%s+default%s+function%s+@',
  'export%s+function%s+@',
  'export%s+default%s+class%s+@',
  'export%s+class%s+@',
  'export%s+const%s+@','export%s+let%s+@','export%s+var%s+@','export%s+enum%s+@',
  'async%s+function%s+@','function%s+@','class%s+@',
  {'const%s+@','let%s+@','var%s+@'}
})
