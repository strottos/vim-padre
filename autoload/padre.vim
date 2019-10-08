" vim: et ts=2 sts=2 sw=2

" This is basic vim plugin boilerplate
let s:save_cpo = &cpoptions
set cpoptions&vim

" Basic setup
let s:Setup = 0

function! padre#Enable()
  if s:Setup
    return
  endif

  call padre#signs#Setup()

  call padre#debugger#Setup()

  let s:Setup = 1
endfunction

function! padre#Disable()
  call padre#debugger#Stop()

  let s:Setup = 0
endfunction

" This is basic vim plugin boilerplate
let &cpoptions = s:save_cpo
unlet s:save_cpo
