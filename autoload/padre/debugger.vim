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
  call padre#socket#Send({"cmd": "run"}, function('padre#debugger#RunCallback'))
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
  call padre#socket#Send({"cmd": "breakpoint", "file": a:file, "line": str2nr(a:line)}, function('padre#debugger#BreakpointCallback'))
endfunction

function! padre#debugger#Breakpoint()
  let l:breakpointAdded = padre#signs#ToggleBreakpoint()

  if !empty(l:breakpointAdded) && padre#socket#Status() is# "open"
    call s:SetBreakpointInDebugger(l:breakpointAdded['line'], l:breakpointAdded['file'])
  endif
endfunction

function! padre#debugger#StepIn()
  call padre#socket#Send({"cmd": "stepIn"}, function('padre#debugger#StepInCallback'))
endfunction

function! padre#debugger#StepOver()
  call padre#socket#Send({"cmd": "stepOver"}, function('padre#debugger#StepOverCallback'))
endfunction

function! padre#debugger#PrintVariable(variable)
  call padre#socket#Send({"cmd": "print", "variable": a:variable}, function('padre#debugger#PrintVariableCallback'))
endfunction

function! padre#debugger#Continue()
  call padre#socket#Send({"cmd": "continue"}, function('padre#debugger#ContinueCallback'))
endfunction

function! padre#debugger#SetVariable(variable)
  let l:value = input("Please enter a value for " . a:variable . ": ")
  call padre#socket#Send({"cmd": "set", "variable": a:variable, "value": l:value}, function('padre#debugger#SetCallback'))
endfunction

""""""""""""""""
" API functions

function! padre#debugger#SignalPADREStarted()
  call padre#debugger#Log(4, 'PADRE Running')
  for l:breakpoint in padre#signs#GetAllBreakpointSigns()
    call s:SetBreakpointInDebugger(l:breakpoint['line'], l:breakpoint['file'])
  endfor
endfunction

function! padre#debugger#RunCallback(channel_id, data)
  if a:data['status'] != 'OK'
    call padre#debugger#Log(2, 'Error: ' . string(a:data))
    return
  endif

  if has_key(a:data, 'pid')
    call padre#debugger#Log(4, 'Process ' . a:data['pid'] . ' Running')
  endif
endfunction

function! padre#debugger#BreakpointCallback(channel_id, data)
  if a:data['status'] == 'OK'
  elseif a:data['status'] == 'PENDING'
    call padre#debugger#Log(4, 'Breakpoint pending')
  else
    call padre#debugger#Log(2, 'Error: ' . string(a:data))
  endif
endfunction

function! padre#debugger#BreakpointSet(fileName, lineNum)
  let l:msg = 'Breakpoint set file=' . a:fileName . ', line=' . a:lineNum
  call padre#debugger#Log(4, l:msg)
endfunction

function! padre#debugger#StepInCallback(channel_id, data)
  if a:data['status'] != 'OK'
    call padre#debugger#Log(2, 'Error: ' . string(a:data))
  endif
endfunction

function! padre#debugger#StepOverCallback(channel_id, data)
  if a:data['status'] != 'OK'
    call padre#debugger#Log(2, 'Error: ' . string(a:data))
  endif
endfunction

function! padre#debugger#ContinueCallback(channel_id, data)
  if a:data['status'] != 'OK'
    call padre#debugger#Log(2, 'Error: ' . string(a:data))
  endif
endfunction

function! padre#debugger#PrintVariableCallback(channel_id, data)
  let l:status = remove(a:data, 'status')
  if l:status != 'OK'
    call padre#debugger#Log(2, 'Error printing variable: ' . string(a:data))
    return
  endif

  let l:variable_name = remove(a:data, 'variable')

  execute "let l:json = system('python -m json.tool', '" . substitute(json_encode(a:data), "'", "''", "g") . "')"
  let l:msg = 'Variable ' . l:variable_name . "=\n" . l:json
  call padre#debugger#Log(4, l:msg)
endfunction

function! padre#debugger#SetCallback(channel_id, data)
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

function! padre#debugger#ProcessExited(exit_code, pid)
  call padre#debugger#Log(4, 'Process ' . a:pid . ' finished with exit code=' . a:exit_code)
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
