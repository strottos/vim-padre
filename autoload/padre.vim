" vim: et ts=2 sts=2 sw=2

" This is basic vim plugin boilerplate
let s:save_cpo = &cpoptions
set cpoptions&vim

" Use Python3 by default
let s:py = has('python3') ? 'py3' : 'py'
let s:pyeval = function(has('python3') ? 'py3eval' : 'pyeval')

" Basic setup
let s:PadreSetup = 0
let s:FileSearchBufferName = ''
let s:JobList = []

function! s:SetUpPython()
  execute s:py 'from api import API'
  execute s:py 'padre_api = API()'
  let s:PadreSetup = 1
endfunction

function! s:Pyeval( eval_string )
  if has('python3')
    return py3eval( a:eval_string )
  endif
  return pyeval( a:eval_string )
endfunction

function! padre#Enable()
  if s:PadreSetup
    return
  endif

  call s:SetUpPython()

  let l:buf_title = 'PADRE'
  let l:options = '["noswapfile", "buftype=nofile", "filetype=PADRE", "nobuflisted", "nomodifiable"]'
  let l:cmd = 'padre_api.create_buffer("' . l:buf_title . '", ' . l:options . ')'
  call s:Pyeval(l:cmd)
endfunction

function! padre#RunningJobs()
  let l:num_jobs_running = 0
  for l:job_id in s:JobList
    if app#job#IsRunning(l:job_id)
      let l:num_jobs_running += 1
    endif
  endfor
  return l:num_jobs_running
endfunction

function! s:stopAllJobs()
  for l:job_id in s:JobList
    call app#job#Stop(l:job_id)
  endfor
  let s:JobList = []
endfunction

function! s:findPadreTab()
  redir => l:tabs
    silent exec 'tabs'
  redir end

  let l:tab_page_nr = 0

  for l:line in split(l:tabs, '\n')
    let l:match = matchlist(l:line, '^Tab page \([1-9][0-9]*\)')
    if !empty(l:match)
      let l:tab_page_nr = l:match[1]
      continue
    endif

    let l:match = matchlist(l:line, '^[ >+]*\(.*\)')
    if !empty(l:match) && l:match[1] == 'PADRE'
      return l:tab_page_nr
    endif
  endfor

  return 0
endfunction

function! s:openPadreTab()
  let l:padre_tab_page_nr = s:findPadreTab()

  if l:padre_tab_page_nr == 0
    tabnew

    let l:buffer_number = s:Pyeval('padre_api.get_buffer("PADRE")')
    execute 'buffer ' . l:buffer_number
  else
    execute l:padre_tab_page_nr . 'tabnext'
  endif
endfunction

function! s:closePadreTab()
  let l:padre_tab_page_nr = s:findPadreTab()

  if l:padre_tab_page_nr != 0
    execute l:padre_tab_page_nr . 'tabnext'
    tabclose
  endif
endfunction

function! padre#Debug()
  call s:stopAllJobs()

  call s:openPadreTab()

  let l:padre_port = s:Pyeval('padre_api.get_unused_localhost_port()')

  call add(s:JobList, app#job#Start('padre --port=' . l:padre_port, {}))
endfunction

function! padre#Stop()
  call s:stopAllJobs()

  call s:closePadreTab()
endfunction

" This is basic vim plugin boilerplate
let &cpoptions = s:save_cpo
unlet s:save_cpo
