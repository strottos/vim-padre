" vim: et ts=2 sts=2 sw=2
"
" layout.vim

let s:PadreData = {}

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

" Takes the first buffer or panics if it doesn't exist
function! padre#layout#OpenTabWithBuffer(buffer_name)
  let s:tabs_containing_buffer = s:GetTabNumbersContainingBufferName(a:buffer_name)

  if tabpagenr() != s:tabs_containing_buffer[0]
    execute s:tabs_containing_buffer[0] . 'tabnext'
  endif
endfunction

function! padre#layout#GetTabNumbersContainingBufferName(name)
  return s:GetTabNumbersContainingBufferName(a:name)
endfunction

function! padre#layout#SetupPadre(padre_number)
  " Start from a window on the right, maybe want this configurable but it's
  " helpful for those that have things like NERDTree automatically on the left
  wincmd l

  " Setup the terminal underneath first
  new
  wincmd j
  resize 10
  wincmd k

  " Then setup the code and logs windows
  let s:PadreData['SourceWin'] = winnr()
  vnew
  wincmd l
  let s:PadreData['DataWin'] = winnr()

  let s:PadreData['DataBufnrs'] = []

  " Setup Threads buffer
  call padre#buffer#CreateForCurrentBuffer('PADRE_Threads_' . a:padre_number, 'PADRE_Threads', 0)
  let s:PadreData['ThreadsBufnr'] = bufnr()
  call add(s:PadreData['DataBufnrs'], bufnr())
  call padre#buffer#SetThreadsPadreKeyBindingsForCurrentBuffer()
  call padre#buffer#SetTogglePadreKeyBindingsForCurrentBuffer()
  call padre#buffer#AppendBuffer("TODO: Populate With Threads", 0)
  augroup vimPadreThreadBuffer
    autocmd!
    autocmd BufEnter <buffer> call padre#debugger#ThreadsBufferEnter()
  augroup END

  " Setup Logs buffer
  call padre#buffer#CreateForCurrentBuffer('PADRE_Logs_' . a:padre_number, 'PADRE_Logs', 0)
  let s:PadreData['LogsBufnr'] = bufnr()
  call add(s:PadreData['DataBufnrs'], bufnr())
  let s:PadreData['CurrentDataBufnrIndex'] = len(s:PadreData['DataBufnrs']) - 1
  call padre#buffer#SetTogglePadreKeyBindingsForCurrentBuffer()

  wincmd h
  wincmd j
endfunction

function! padre#layout#GetSourceWindow()
  return s:PadreData['SourceWin']
endfunction

function! padre#layout#GetDataWindow()
  return s:PadreData['DataWin']
endfunction

function! padre#layout#GetDataBufnr(buf_type)
  return s:PadreData[a:buf_type . 'Bufnr']
endfunction

function! padre#layout#Toggle()
  " TODO: Check we're in the right window, assume for now

  if s:PadreData['CurrentDataBufnrIndex'] == len(s:PadreData['DataBufnrs']) - 1
    let s:PadreData['CurrentDataBufnrIndex'] = 0
  else
    let s:PadreData['CurrentDataBufnrIndex'] += 1
  endif

  let l:index = s:PadreData['CurrentDataBufnrIndex']
  execute 'buffer ' . s:PadreData['DataBufnrs'][l:index]
endfunction
