" vim: et ts=2 sts=2 sw=2

" This is basic vim plugin boilerplate
let s:save_cpo = &cpoptions
set cpoptions&vim

" Basic setup
let s:PadreSetup = 0
let s:FileSearchBufferName = ''
let s:PadreJobId = 0

function! padre#Enable()
  if s:PadreSetup
    return
  endif

  call padre#python#Setup()

  let l:buf_title = 'PADRE'
  let l:options = ['noswapfile', 'buftype=nofile', 'filetype=PADRE', 'nobuflisted', 'nomodifiable']
  call padre#buffer#Create(l:buf_title, l:options)

  let s:PadreSetup = 1
endfunction

function! padre#Debug()
  call padre#job#Stop(s:PadreJobId)

  call padre#layout#OpenTabWithBuffer('PADRE', 0)

  let l:padre_port = padre#util#GetUnusedLocalhostPort()

  let s:PadreJobId = padre#job#Start('padre --port=' . l:padre_port, {})
endfunction

function! padre#Stop()
  call padre#job#StopAllJobs()

  call padre#layout#CloseTabsWithBuffer('PADRE')
endfunction

" This is basic vim plugin boilerplate
let &cpoptions = s:save_cpo
unlet s:save_cpo
