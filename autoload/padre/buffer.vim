" vim: et ts=2 sts=2 sw=2
"
" buffer.vim
"
" Libraries for manipulating buffers.

" Create a PADRE style buffer
function! padre#buffer#CreateForCurrentBuffer(name, filetype, writeable)
  execute "silent edit " . a:name

  setlocal noswapfile
  setlocal buftype=nofile
  execute "setlocal filetype=" . a:filetype
  setlocal nobuflisted
  if a:writeable == 0
    setlocal nomodifiable
  endif
endfunction

function! padre#buffer#SetMainPadreKeyBindingsForCurrentBuffer()
  nnoremap <silent> <buffer> r :PadreRun<cr>
  nnoremap <silent> <buffer> S :PadreStepIn<cr>
  nnoremap <silent> <buffer> s :PadreStepOver<cr>
  vnoremap <silent> <buffer> p y:PadrePrintVariable <C-R>"<cr>
  nnoremap <silent> <buffer> C :PadreContinue<cr>
  nnoremap <silent> <buffer> ZZ :PadreStop<cr>
endfunction

function! padre#buffer#UnsetPadreKeyBindingsForCurrentBuffer()
  nnoremap <silent> <buffer> r r
  nnoremap <silent> <buffer> S S
  nnoremap <silent> <buffer> s s
  vnoremap <silent> <buffer> p p
  nnoremap <silent> <buffer> C C
  nnoremap <silent> <buffer> ZZ ZZ
endfunction

function! padre#buffer#AppendBuffer(text, modifiable)
  let l:bufnr = bufnr('%')

  let l:should_scroll = getpos('.')[1] == line('$')

  if a:modifiable == 0
    call setbufvar(l:bufnr, '&modifiable', 1)
  endif

  for l:str in split(a:text, '\n')
    call setbufline(l:bufnr, '$', [l:str, ''])
  endfor

  if a:modifiable == 0
    call setbufvar(l:bufnr, '&modifiable', 0)
  endif

  " Scroll to bottom if we were already at bottom
  if l:should_scroll
    normal G
  endif
endfunction

function! padre#buffer#ReplaceBuffer(text, modifiable)
  let l:bufnr = bufnr('%')

  if a:modifiable == 0
    call setbufvar(l:bufnr, '&modifiable', 1)
  endif

  call padre#buffer#ClearBuffer(1)
  return padre#buffer#AppendBuffer(a:text, 1)

  if a:modifiable == 0
    call setbufvar(l:bufnr, '&modifiable', 0)
  endif
endfunction

function! padre#buffer#ClearBuffer(modifiable)
  let l:bufnr = bufnr('%')

  if a:modifiable == 0
    call setbufvar(l:bufnr, '&modifiable', 1)
  endif

  normal 1GdG

  if a:modifiable == 0
    call setbufvar(l:bufnr, '&modifiable', 0)
  endif
endfunction
