" vim: et ts=2 sts=2 sw=2
"
" util.vim

function! padre#util#GetUnusedLocalhostPort()
  return padre#python#CallAPI('get_unused_localhost_port()')
endfunction
