" vim: et ts=2 sts=2 sw=2
"
" buffer.vim
"
" Libraries for manipulating buffers.

let s:BufferNames = {}
let s:BufferNums = {}

function! s:LoadBuffer(name)
  let l:buffer_number = padre#buffer#GetBufNumForBufName(a:name)
  if l:buffer_number == v:none
    let l:buffer_number = bufnr(a:name)
  endif
  execute 'buffer ' . l:buffer_number
endfunction

" Create a buffer
function! padre#buffer#Create(name, filetype, writeable)
  let l:current_winnr = winnr()
  let l:current_bufnr = bufnr('%')
  let l:current_tabpagenr = tabpagenr()

  tabnew
  execute "silent edit " . a:name

  setlocal noswapfile
  setlocal buftype=nofile
  execute "setlocal filetype=" . a:filetype
  setlocal nobuflisted
  if a:writeable == 0
    setlocal nomodifiable
  endif
  let l:new_bufnr = bufnr('%')

  quit

  execute l:current_tabpagenr . " tabnext"
  execute l:current_winnr . "wincmd w"
  execute "buffer " . l:current_bufnr

  execute 'let s:BufferNames.' . l:new_bufnr . ' = "' . a:name . '"'
  execute 'let s:BufferNums.' . a:name . ' = ' . l:new_bufnr

  return l:new_bufnr
endfunction

function! padre#buffer#SetOnlyWriteableAtBottom(name)
  tabnew

  call s:LoadBuffer(a:name)

  setlocal modifiable

  let l:cursor_to_bottom_line = ''
  for l:letter in ['a', 'C', 'D', 'i', 'P', 'p' ,'R', 'r', 'X', 'x']
    execute 'nnoremap <silent> <buffer> ' . l:letter . ' :call cursor("$", getpos(".")[2])<cr>' . l:letter
  endfor
  nnoremap <silent> <buffer> A GA
  " TODO: c
  nnoremap <silent> <buffer> dd GD
  nnoremap <silent> <buffer> I GI
  nnoremap <silent> <buffer> O <nop>
  nnoremap <silent> <buffer> o <nop>
  nnoremap <silent> <buffer> S GS
  " TODO: s
  " TODO: Up arrow
  " TODO: Down arrow

  quit
endfunction

function! padre#buffer#SetMainPadreKeyBindings(name)
  let l:current_tabpagenr = tabpagenr()

  tabnew

  call s:LoadBuffer(a:name)

  nnoremap <silent> <buffer> r :PadreRun<cr>
  nnoremap <silent> <buffer> S :PadreStepIn<cr>
  nnoremap <silent> <buffer> s :PadreStepOver<cr>
  vnoremap <silent> <buffer> p y:PadrePrintVariable <C-R>"<cr>
  nnoremap <silent> <buffer> C :PadreContinue<cr>
  nnoremap <silent> <buffer> ZZ :PadreStop<cr>

  quit

  execute l:current_tabpagenr . " tabnext"
endfunction

function! padre#buffer#UnsetPadreKeyBindings(name)
  let l:current_tabpagenr = tabpagenr()

  tabnew

  call s:LoadBuffer(a:name)

  nnoremap <silent> <buffer> r r
  nnoremap <silent> <buffer> S S
  nnoremap <silent> <buffer> s s
  vnoremap <silent> <buffer> p p
  nnoremap <silent> <buffer> C C
  nnoremap <silent> <buffer> ZZ ZZ

  quit

  execute l:current_tabpagenr . " tabnext"
endfunction

function! padre#buffer#GetBufNameForBufNum(num)
  let l:num = string(a:num)
  if index(keys(s:BufferNames), l:num) != -1
    return s:BufferNames[l:num]
  endif
  return ''
endfunction

function! padre#buffer#GetBufNumForBufName(name)
  return s:BufferNums[a:name]
endfunction

function! padre#buffer#GetBufferNumbers()
  return values(s:BufferNums)
endfunction

function! padre#buffer#GetBufferNames()
  return values(s:BufferNames)
endfunction

function! padre#buffer#LoadBufferName(name)
  execute 'buffer ' . padre#buffer#GetBufNumForBufName(a:name)
endfunction

function! padre#buffer#ReadBuffer(name)
  let l:bufnr = padre#buffer#GetBufNumForBufName(a:name)
  return getbufline(l:bufnr, 1, '$')
endfunction

function! padre#buffer#AppendBufferString(name, text)
  let l:bufnr = padre#buffer#GetBufNumForBufName(a:name)
  let l:was_modifiable = getbufvar(l:bufnr, '&modifiable')

  let l:text = split(a:text, "\n")
  if len(l:text) > 0
    let l:text[0] = getbufline(l:bufnr, '$')[0] . l:text[0]
  endif

  if l:was_modifiable == 0
    call setbufvar(l:bufnr, '&modifiable', 1)
  endif

  call setbufline(l:bufnr, '$', l:text)

  if l:was_modifiable == 0
    call setbufvar(l:bufnr, '&modifiable', 0)
  endif
endfunction

function! padre#buffer#AppendBuffer(name, text)
  let l:bufnr = padre#buffer#GetBufNumForBufName(a:name)
  let l:was_modifiable = getbufvar(l:bufnr, '&modifiable')

  let l:text = a:text + ['']

  let l:tab_has_buffer = padre#layout#CurrentTabContainsBuffer(a:name)

  if l:tab_has_buffer
    let l:current_window = winnr()
    call padre#layout#FindBufferWindowWithinTab(a:name)
    let l:should_scroll = getpos('.')[1] == line('$')
  endif

  if l:was_modifiable == 0
    call setbufvar(l:bufnr, '&modifiable', 1)
  endif

  call setbufline(l:bufnr, '$', l:text)

  if l:was_modifiable == 0
    call setbufvar(l:bufnr, '&modifiable', 0)
  endif

  if l:tab_has_buffer
    if l:should_scroll
      normal G
    endif
    execute l:current_window . ' wincmd w'
  endif
endfunction

function! padre#buffer#ReplaceBuffer(name, text)
  call padre#buffer#ClearBuffer(a:name)
  return padre#buffer#AppendBuffer(a:name, a:text)
  "return padre#python#CallAPI('replace_buffer("' . a:name . '", "' . a:line_from . '", "' . a:line_to . '", ["' . join(a:text, '","') . '"])')
endfunction

function! padre#buffer#ClearBuffer(name)
  tabnew
  execute 'buffer ' . padre#buffer#GetBufNumForBufName(a:name)

  let l:was_modifiable = &modifiable

  if l:was_modifiable == 0
    setlocal modifiable
  endif

  normal 1GdG

  if l:was_modifiable == 0
    setlocal nomodifiable
  endif

  quit
endfunction
