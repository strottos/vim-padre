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
      let l:tab_page_nr = str2nr(l:match[1])
      continue
    endif

    let l:match = matchlist(l:line, '^[ >+]*\(.*\)')
    if !empty(l:match) && l:match[1] == a:name && index(l:ret, l:tab_page_nr) == -1
      call add(l:ret, l:tab_page_nr)
    endif
  endfor

  return l:ret
endfunction

" TODO: Work out what to do with selecting a window for the buffer when we have
" multiple windows. Probably nothing. Make a test if so?
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

function! padre#layout#CurrentTabContainsBuffer(name)
  return index(s:GetTabNumbersContainingBufferName(a:name), tabpagenr()) != -1
endfunction

function! padre#layout#CloseTabsWithBuffer(buffer_name)
  for l:tab_num in reverse(s:GetTabNumbersContainingBufferName(a:buffer_name))
    execute l:tab_num . 'tabnext'
    while tabpagenr() == l:tab_num
      quit
    endwhile
  endfor
endfunction

function! padre#layout#GetBuffersInTab()
  let l:ret = []
  for l:winid in gettabinfo(tabpagenr())[0].windows
    call add(l:ret, getwininfo(l:winid)[0].bufnr)
  endfor
  return l:ret
endfunction

function! padre#layout#FindBufferWindowWithinTab(bufName)
  let l:original_winnr = winnr()

  if winnr('$') == 1
    return
  endif

  for l:num in range(1, winnr('$'))
    wincmd w
    if bufname('%') == a:bufName
      return
    endif
  endfor

  execute l:original_winnr . ' wincmd'
endfunction

" Arguments:
"   - pos: Position of the window to be created, one of t,l,r,b for top, left,
"       right, bottom
"   - size: Number indicated the size of the window created (TODO: Make size
"       optional)
"   - buffer_name: The name of the buffer to show
"   - create_new: Defaults to 1 indicating we always create a new window. If
"       set to 0 we look for a tab with the existing buffer and open that,
"       otherwise we create one
function! padre#layout#AddWindowToTab(pos, size, ...)
  let l:create_new = 1

  if a:0 > 0
    let l:buffer_name = a:1
  endif

  if a:0 == 2 && a:2 == 0
    for l:buf_number in padre#layout#GetBuffersInTab()
      if padre#buffer#GetBufNameForBufNum(l:buf_number) ==# l:buffer_name
        let l:create_new = 0
        break
      endif
    endfor
  endif

  if l:create_new == 1
    if a:pos == 't'
      execute a:size . 'split'
    elseif a:pos == 'b'
      let l:size = max([winheight(winnr()) - a:size, 10])
      execute l:size . 'split'
      wincmd j
    elseif a:pos == 'l'
      execute a:size . 'vsplit'
    elseif a:pos == 'r'
      let l:size = max([winwidth(winnr()) - a:size, 20])
      execute l:size . 'vsplit'
      wincmd l
    endif

    if a:0 > 0
      execute 'buffer ' . padre#buffer#GetBufNumForBufName(l:buffer_name)
    endif

    if a:pos == 't'
      wincmd j
    elseif a:pos == 'b'
      wincmd k
    elseif a:pos == 'l'
      wincmd l
    elseif a:pos == 'r'
      wincmd h
    endif
  endif

  return l:create_new
endfunction
