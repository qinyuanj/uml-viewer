local Source=require('uml.source')
return Source.file_source({
  {'function%s+@:[A-Za-z_][%w_]*','function%s+@'},
  {'local%s+function%s+@:[A-Za-z_][%w_]*','local%s+function%s+@'},
  '@%s*=%s*function','local%s+@%s*=%s*function'
})
