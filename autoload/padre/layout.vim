" vim: et ts=2 sts=2 sw=2
"
" layout.vim

function! s:GetTabNumbersContainingBufferName(name)
  redir => l:tabs
    silent exec 'tabs'
  redir end

  let l:ret = []
  let l:tab_page_nr = 0

  for l:line in split(l:tabs, '\n')
    let l:match = matchlist(l:line, '^Tab page \([1-9][0-9]*\)')
    if !empty(l:match)
      let l:tab_page_nr = l:match[1]
      continue
    endif

    let l:match = matchlist(l:line, '^[ >+]*\(.*\)')
    if !empty(l:match) && l:match[1] == a:name
      call add(l:ret, str2nr(l:tab_page_nr))
    endif
  endfor

  return l:ret
endfunction

" TODO: Work out what to do with selecting a window for the buffer when we have
" multiple windows
function! padre#layout#OpenTabWithBuffer(buffer_name, create_new)
  let l:create_new = a:create_new

  if l:create_new == 0
    let s:tabs_containing_buffer = s:GetTabNumbersContainingBufferName(a:buffer_name)

    if empty(s:tabs_containing_buffer)
      let l:create_new = 1
    else
      execute s:tabs_containing_buffer[0] . 'tabnext'
    endif
  endif

  if l:create_new == 1
    tabnew

    execute 'buffer ' . padre#buffer#GetBufNumForBufName(a:buffer_name)
  endif
endfunction

function! padre#layout#GetTabNumbersContainingBufferName(name)
  return s:GetTabNumbersContainingBufferName(a:name)
endfunction

function! padre#layout#CloseTabsWithBuffer(buffer_name)
  for l:tab_num in reverse(s:GetTabNumbersContainingBufferName(a:buffer_name))
    execute l:tab_num . 'tabnext'
    while tabpagenr() == l:tab_num
      quit
    endwhile
  endfor
endfunction
