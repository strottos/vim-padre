" vim: et ts=2 sts=2 sw=2
"
" buffer.vim
"
" Libraries for manipulating buffers.

function! s:LoadBuffer(name)
  tabnew
  let l:buffer_number = padre#buffer#GetBufNumForBufName(a:name)
  if l:buffer_number == v:none
    let l:buffer_number = bufnr(a:name)
  endif
  execute 'buffer ' . l:buffer_number
endfunction

" Create a buffer
"
" Arguments:
"   - writeable: Takes one of the following
"       0: Totally non-modifiable (without you hacking it)
"       1: Totally modifiable
"       2: Only the last line modifiable similar to a terminal
function! padre#buffer#Create(name, filetype, writeable)
  let l:options = ['setlocal noswapfile', 'setlocal buftype=nofile', 'setlocal filetype=' . a:filetype, 'setlocal nobuflisted']
  if a:writeable == 0
    call add(l:options, 'setlocal nomodifiable')
  endif
  return padre#python#CallAPI('create_buffer("' . a:name . '", ["' . join(l:options, '","') . '"])')
endfunction

function! padre#buffer#SetOnlyWriteableAtBottom(name)
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
  call s:LoadBuffer(a:name)

  nnoremap <silent> <buffer> r :PadreRun<cr>
  nnoremap <silent> <buffer> s :PadreStepIn<cr>
  nnoremap <silent> <buffer> n :PadreStepOver<cr>
  vnoremap <silent> <buffer> p y:PadrePrintVariable <C-R>"<cr>
  nnoremap <silent> <buffer> C :PadreContinue<cr>

  quit
endfunction

function! padre#buffer#UnsetPadreKeyBindings(name)
  call s:LoadBuffer(a:name)

  nnoremap <silent> <buffer> r r
  nnoremap <silent> <buffer> s s
  nnoremap <silent> <buffer> n n
  vnoremap <silent> <buffer> p p
  nnoremap <silent> <buffer> C C

  quit
endfunction

function! padre#buffer#GetBufNameForBufNum(num)
  return padre#python#CallAPI('get_buffer_name(' . a:num . ')')
endfunction

function! padre#buffer#GetBufNumForBufName(name)
  return padre#python#CallAPI('get_buffer_number("' . a:name . '")')
endfunction

function! padre#buffer#LoadBufferName(name)
  execute 'buffer ' . padre#buffer#GetBufNumForBufName(a:name)
endfunction

function! padre#buffer#ReadBuffer(name)
  return padre#python#CallAPI('read_buffer("' . a:name . '")')
endfunction

function! padre#buffer#PrependBufferString(name, line, text)
  return padre#python#CallAPI('prepend_buffer("' . a:name . '", "' . a:line . '", "' . substitute(a:text, '"', '\\"', 'g') . '")')
endfunction

function! padre#buffer#AppendBufferString(name, line, text)
  return padre#python#CallAPI('append_buffer("' . a:name . '", "' . a:line . '", "' . substitute(substitute(substitute(a:text, '\\', '\\\\', 'g'), '"', '\\"', 'g'), '', '\\n', 'g') . '")')
endfunction

function! padre#buffer#PrependBufferList(name, line, text)
  return padre#python#CallAPI('prepend_buffer("' . a:name . '", "' . a:line . '", ["' . join(a:text, '","') . '"])')
endfunction

function! padre#buffer#AppendBufferList(name, line, text)
  return padre#python#CallAPI('append_buffer("' . a:name . '", "' . a:line . '", ["' . join(a:text, '","') . '"])')
endfunction

function! padre#buffer#ReplaceBufferList(name, line_from, line_to, text)
  return padre#python#CallAPI('replace_buffer("' . a:name . '", "' . a:line_from . '", "' . a:line_to . '", ["' . join(a:text, '","') . '"])')
endfunction

function! padre#buffer#ClearBuffer(name)
  return padre#python#CallAPI('clear_buffer("' . a:name . '")')
endfunction
