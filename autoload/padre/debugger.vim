" vim: et ts=2 sts=2 sw=2
"
" debugger.vim
"
" Responsible for interfacing with and communicating with the PADRE debugger
" process

let s:PadrePluginRoot = expand('<sfile>:p:h:h:h')
let s:CurrentFileLoaded = ''
let s:PresentDirectory = ''
let s:PadreNumber = 0

function! padre#debugger#Setup()
  let s:PresentDirectory = expand('%:p:h')
endfunction

function! padre#debugger#Debug(...)
  let s:PadreNumber += 1

  let l:program = ''
  let l:padre_host = 'localhost'
  let l:padre_port = 0
  let l:debugger = ''
  let l:debugger_type = ''

  let l:args = a:000
  let l:process_vim_args = 1

  while len(l:args) > 0
    let l:arg = l:args[0]
    let l:args = l:args[1:]

    let l:match = matchlist(l:arg, '^--debugger=\([^ ]*\)$')
    if !empty(l:match) && l:process_vim_args == 1
      let l:debugger = l:match[1]
      continue
    endif

    let l:match = matchlist(l:arg, '^-d=\([^ ]*\)$')
    if !empty(l:match) && l:process_vim_args == 1
      let l:debugger = l:match[1]
      continue
    endif

    let l:match = matchlist(l:arg, '^--type=\([^ ]*\)$')
    if !empty(l:match) && l:process_vim_args == 1
      let l:debugger_type = l:match[1]
      continue
    endif

    let l:match = matchlist(l:arg, '^-t=\([^ ]*\)$')
    if !empty(l:match) && l:process_vim_args == 1
      let l:debugger_type = l:match[1]
      continue
    endif

    let l:match = matchlist(l:arg, '^--connect=\([^ ]*\):\([0-9]*\)$')
    if !empty(l:match) && l:process_vim_args == 1
      let l:padre_host = match[1]
      let l:padre_port = match[2]
      continue
    endif

    let l:match = matchlist(l:arg, '^--$')
    if !empty(l:match) && l:process_vim_args == 1
      let l:process_vim_args = 0
      continue
    endif

    if l:program == ''
      let l:program = l:arg
    else
      let l:program .= ' ' . l:arg
    endif
  endwhile

  if l:program == '' && l:padre_port == 0
    if get(g:, 'PadreDebugProgram', '') != ''
      let l:program = g:PadreDebugProgram
    else
      echoerr 'PADRE Program not found, please specify'
      return
    endif
  endif

  "" Guarantee we're starting fresh, if the current buffer is empty
  if winnr('$') != 1 || line('$') != 1 || !empty(getbufline(bufnr('%'), 1)[0]) || &modified == 1
    tabnew
  endif

  call padre#layout#SetupPadre(s:PadreNumber)

  if l:padre_port == 0
    " TODO: Check for errors and report
    let l:command = s:PadrePluginRoot . '/padre/target/debug/padre'
    if l:debugger != ''
      let l:command .= ' --debugger=' . l:debugger
    endif
    if l:debugger_type != ''
      let l:command .= ' --type=' . l:debugger_type
    endif
    let l:command .= ' -- ' . l:program

    execute 'terminal ++curwin ' . l:command

    let l:timeout = get(g:, 'PadreStartupTimeout', 10)

    for i in range(float2nr(l:timeout / 0.25))
      sleep 250ms

      let l:connection_line = ''

      let l:connection_line = getline(1)
      let l:match = matchlist(l:connection_line, '^Listening on \([^ ]*\):\([0-9]*\)$')
      if !empty(l:match)
        let l:padre_host = l:match[1]
        let l:padre_port = l:match[2]
        break
      endif
    endfor
  endif

  if l:padre_port == 0
    echom "Can't connect to PADRE, unknown port"
    return
  endif

  call padre#socket#Connect(l:padre_host, l:padre_port, function('padre#debugger#SignalPADREStarted'))

  wincmd k
  wincmd l
  call padre#buffer#SetMainPadreKeyBindingsForCurrentBuffer()
  wincmd h
  call padre#buffer#SetMainPadreKeyBindingsForCurrentBuffer()
endfunction

function! padre#debugger#Run()
  call padre#socket#Send({"cmd": "run"}, function('padre#debugger#GenericCallback'))
endfunction

function! padre#debugger#Stop()
  if s:PadreNumber == 0
    return
  endif

  let l:current_tabpagenr = tabpagenr()

  call padre#layout#OpenTabWithBuffer('PADRE_Logs_' . s:PadreNumber)

  let l:padre_tabpagenr = tabpagenr()

  for i in range(winnr('$'))
    quit!
  endfor

  if l:current_tabpagenr != tabpagenr() && l:current_tabpagenr != l:padre_tabpagenr
    execute l:current_tabpagenr . 'tabnext'
  endif

  call padre#socket#Close()
endfunction

function! s:SetBreakpointInDebugger(line, file)
  call padre#socket#Send({"cmd": "breakpoint", "file": a:file, "line": str2nr(a:line)}, function('padre#debugger#GenericCallback'))
endfunction

function! padre#debugger#Breakpoint()
  let l:breakpointAdded = padre#signs#ToggleBreakpoint()

  if padre#socket#Status() is# "open"
    if !empty(l:breakpointAdded)
      call s:SetBreakpointInDebugger(l:breakpointAdded['line'], l:breakpointAdded['file'])
    else
      let l:file = expand('%')
      let l:line = getpos('.')[1]
      call padre#socket#Send({"cmd": "unbreakpoint", "file": l:file, "line": getpos('.')[1]}, function('padre#debugger#GenericCallback'))
    endif
  endif
endfunction

function! padre#debugger#StepIn(count)
  call padre#socket#Send({"cmd": "stepIn", "count": a:count}, function('padre#debugger#GenericCallback'))
endfunction

function! padre#debugger#StepOver(count)
  call padre#socket#Send({"cmd": "stepOver", "count": a:count}, function('padre#debugger#GenericCallback'))
endfunction

function! padre#debugger#StepOut()
  call padre#socket#Send({"cmd": "stepOut"}, function('padre#debugger#GenericCallback'))
endfunction

function! padre#debugger#PrintVariable(variable)
  call padre#socket#Send({"cmd": "print", "variable": a:variable}, function('padre#debugger#PrintVariableCallback'))
endfunction

function! padre#debugger#Continue()
  call padre#socket#Send({"cmd": "continue"}, function('padre#debugger#GenericCallback'))
endfunction

""""""""""""""""
" API functions

function! padre#debugger#SignalPADREStarted()
  call padre#debugger#Log(4, 'PADRE Running')
  for l:breakpoint in padre#signs#GetAllBreakpointSigns()
    call s:SetBreakpointInDebugger(l:breakpoint['line'], l:breakpoint['file'])
  endfor
endfunction

function! padre#debugger#GenericCallback(channel_id, data)
  if a:data['status'] != 'OK'
    call padre#debugger#Log(2, 'Error: ' . string(a:data))
  endif
endfunction

function! padre#debugger#JumpToPosition(file, line)
  let l:msg = 'Stopped file=' . a:file . ' line=' . a:line
  call padre#debugger#Log(4, l:msg)

  if a:file[0] == '/'
    let l:fileToLoad = a:file
  else
    let l:fileToLoad = s:PresentDirectory . '/' . findfile(a:file, s:PresentDirectory . '/**')
  endif

  call padre#layout#OpenTabWithBuffer('PADRE_Logs_' . s:PadreNumber)

  let l:current_window = winnr()
  let l:source_window = padre#layout#GetSourceWindow()

  if l:current_window != l:source_window
    execute l:source_window . ' wincmd w'
  endif

  if l:fileToLoad != expand('%:p')
    let l:current_tabpagenr = tabpagenr()

    if filereadable(l:fileToLoad)
      echom "Opening new file " . l:fileToLoad
      execute 'view! ' . l:fileToLoad

      let s:CurrentFileLoaded = l:fileToLoad
    else
      echom "WARNING: Can't open file " . l:fileToLoad
    endif

    call padre#buffer#SetMainPadreKeyBindingsForCurrentBuffer()

    if l:current_tabpagenr != tabpagenr()
      execute l:current_tabpagenr . 'tabnext'
    endif
  endif

  call padre#signs#ReplaceCodePointer(a:line)

  execute 'normal ' . a:line . 'G'

  if l:current_window != l:source_window
    execute l:current_window . ' wincmd w'
  endif

  redraw
endfunction

function! padre#debugger#PrintVariableCallback(channel_id, data)
  if a:data['status'] != 'OK'
    call padre#debugger#Log(2, 'Error: ' . string(a:data))
  endif

  let s:text = 'Variable (' . a:data['type'] . ') ' . a:data['variable'] . '=' . a:data['value']

  let l:current_tabpagenr = tabpagenr()

  call padre#layout#OpenTabWithBuffer('PADRE_Logs_' . s:PadreNumber)

  let l:current_window = winnr()
  let l:logs_window = padre#layout#GetLogsWindow()

  if l:current_window != l:logs_window
    execute l:logs_window . ' wincmd w'
  endif

  call padre#buffer#AppendBuffer(strftime('%y/%m/%d %H:%M:%S ') . s:text, 0)

  if l:current_window != l:logs_window
    execute l:current_window . ' wincmd w'
  endif

  if l:current_tabpagenr != tabpagenr()
    execute l:current_tabpagenr . 'tabnext'
  endif

  redraw
endfunction

function! padre#debugger#Log(level, text)
  let l:log_level_set = get(g:, 'PadreLogLevel', 4)
  let l:level = ''

  if a:level > l:log_level_set
    return
  endif

  if a:level == 1
    let l:level = '(CRITICAL): '
  elseif a:level == 2
    let l:level = '(ERROR): '
  elseif a:level == 3
    let l:level = '(WARN): '
  elseif a:level == 4
    let l:level = '(INFO): '
  elseif a:level == 5
    let l:level = '(DEBUG): '
  endif

  let l:current_tabpagenr = tabpagenr()

  call padre#layout#OpenTabWithBuffer('PADRE_Logs_' . s:PadreNumber)

  let l:current_window = winnr()
  let l:logs_window = padre#layout#GetLogsWindow()

  if l:current_window != l:logs_window
    execute l:logs_window . ' wincmd w'
  endif

  call padre#buffer#AppendBuffer(strftime('%y/%m/%d %H:%M:%S ') . l:level . a:text, 0)

  if l:current_window != l:logs_window
    execute l:current_window . ' wincmd w'
  endif

  if l:current_tabpagenr != tabpagenr()
    execute l:current_tabpagenr . 'tabnext'
  endif

  redraw
endfunction
