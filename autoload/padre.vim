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

"  if has_key(g:, 'PadrePreprocessingCommands')
"    call padre#buffer#ReplaceBuffer('PADRE_Preprocessing', g:PadrePreprocessingCommands)
"  endif

  let s:Setup = 1
endfunction

function! padre#Disable()
  call padre#debugger#Stop()

  for l:buffer_name in ['PADRE_Main'] ", 'PADRE_Preprocessing']
    let l:buffer_number = padre#buffer#GetBufNumForBufName(l:buffer_name)
    if matchstr(l:buffer_number, '\d')
      execute 'bwipeout! ' l:buffer_number
    endif
  endfor

  let s:Setup = 0
endfunction

" This is basic vim plugin boilerplate
let &cpoptions = s:save_cpo
unlet s:save_cpo
