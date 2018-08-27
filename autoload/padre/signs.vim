" vim: et ts=2 sts=2 sw=2
"
" signs.vim
"
" Responsible for toggling and reporting on breakpoints in files and
" displaying the current position of the debugger in code
"
" NB: Does not communicate with PADRE

" Arbitrary number so no clashes with other plugins (hopefully)
" TODO: Find a better solution
let s:NextSignId = 1578311
let s:CurrentPointerId = 0

function! padre#signs#Setup()
  let [guibg, ctermbg] = s:get_background_colors('SignColumn')

  execute 'highlight PadreBreakpointHighlight guifg=#ff0000 guibg=' . guibg . ' ctermfg=red ctermbg=' . ctermbg
  execute 'highlight PadreDebugPointerHighlight guifg=#00ff00 guibg=' . guibg . ' ctermfg=green ctermbg=' . ctermbg

  sign define PadreBreakpoint text=() texthl=PadreBreakpointHighlight
  sign define PadreDebugPointer text=-> texthl=PadreDebugPointerHighlight
endfunction

function! s:get_background_colors(group) abort
  redir => highlight
  silent execute 'silent highlight ' . a:group
  redir END

  let l:link_matches = matchlist(highlight, 'links to \(\S\+\)')
  if len(l:link_matches) > 0 " follow the link
    return s:get_background_colors(link_matches[1])
  endif

  let l:ctermbg = s:match_highlight(highlight, 'ctermbg=\([0-9A-Za-z]\+\)')
  let l:guibg   = s:match_highlight(highlight, 'guibg=\([#0-9A-Za-z]\+\)')
  return [l:guibg, l:ctermbg]
endfunction

function! s:match_highlight(highlight, pattern) abort
  let matches = matchlist(a:highlight, a:pattern)
  if len(matches) == 0
    return 'NONE'
  endif
  return matches[1]
endfunction

function! padre#signs#ToggleBreakpoint()
  let l:file = expand('%')
  let l:line = getpos('.')[1]

  let l:breakpointId = s:LineHasBreakpoint(l:file, l:line)
  if l:breakpointId == 0
    execute 'sign place ' . s:NextSignId . ' line=' . l:line . ' name=PadreBreakpoint file=' . l:file
    let s:NextSignId += 1
    return {'file': l:file, 'line': '' . l:line}
  endif

  execute 'sign unplace ' . l:breakpointId . ' file=' . l:file
  return {}
endfunction

function! s:ReadSignsOutput(signs, name)
  let l:ret = []

  for l:line in split(a:signs, '\n')
    let l:match = matchlist(l:line, 'Signs for \(\S\+\):$')
    if len(l:match) != 0
      let l:filename = l:match[1]
    endif
    let l:match = matchlist(l:line, '^    line=\(\d\+\) * id=\(\d\+\) * name=' . a:name . '$')
    if len(l:match) != 0
      call add(l:ret, {'file': l:filename, 'line': l:match[1]})
    endif
  endfor

  return l:ret
endfunction

function! s:LineHasBreakpoint(file, line)
  redir => l:signs
    silent exec 'sign place file=' . a:file
  redir end

  for l:line in split(l:signs, '\n')
    let l:match = matchlist(l:line, '^    line=' . a:line . ' * id=\(\d\+\) * name=PadreBreakpoint$')
    if len(l:match)
      return match[1]
    endif
  endfor

  return 0
endfunction

function! padre#signs#GetAllBreakpointSignsForFile(file)
  redir => l:signs
    silent exec 'sign place file=' . a:file
  redir end

  return s:ReadSignsOutput(l:signs, 'PadreBreakpoint')
endfunction

function! padre#signs#GetAllBreakpointSigns()
  redir => l:signs
    silent exec 'sign place'
  redir end

  return s:ReadSignsOutput(l:signs, 'PadreBreakpoint')
endfunction

function! padre#signs#ReplaceCodePointer(line)
  if s:CurrentPointerId != 0
    execute 'sign unplace ' . s:CurrentPointerId
  endif

  let s:CurrentPointerId = s:NextSignId
  let s:NextSignId += 1

  if a:line != 0
    execute 'sign place ' . s:CurrentPointerId . ' line=' . a:line . ' name=PadreDebugPointer buffer=' . bufnr('%')
  endif
endfunction
