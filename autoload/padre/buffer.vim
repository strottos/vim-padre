" vim: et ts=2 sts=2 sw=2
"
" buffer.vim
"
" Libraries for manipulating buffers.

" Create a buffer
function! padre#buffer#Create(name, options)
  return padre#python#CallAPI('create_buffer("' . a:name . '", ["' . join(a:options, '","') . '"])')
endfunction

function! padre#buffer#GetBufNameForBufNum(num)
  return padre#python#CallAPI('get_buffer_name(' . a:num . ')')
endfunction

function! padre#buffer#GetBufNumForBufName(name)
  return padre#python#CallAPI('get_buffer_number("' . a:name . '")')
endfunction

function! padre#buffer#PrependBufferString(name, line, text)
  return padre#python#CallAPI('prepend_buffer("' . a:name . '", "' . a:line . '", "' . a:text . '")')
endfunction

function! padre#buffer#AppendBufferString(name, line, text)
  return padre#python#CallAPI('append_buffer("' . a:name . '", "' . a:line . '", "' . a:text . '")')
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
