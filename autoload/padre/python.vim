" vim: et ts=2 sts=2 sw=2
"
" python.vim
"
" Calls the python API in pythonx

" Use Python3 by default
let s:py = has('python3') ? 'py3' : 'py'
let s:pyeval = function(has('python3') ? 'py3eval' : 'pyeval')

function! s:Pyeval( eval_string )
  if has('python3')
    return py3eval( a:eval_string )
  endif
  return pyeval( a:eval_string )
endfunction

function! s:SetUpPython()
  execute s:py 'from api import API'
  execute s:py 'padre_api = API()'
endfunction

function! padre#python#Setup()
  call s:SetUpPython()
endfunction

function! padre#python#CallAPI(cmd)
  return s:Pyeval('padre_api.' . a:cmd)
endfunction
