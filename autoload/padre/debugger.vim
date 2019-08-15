" vim: et ts=2 sts=2 sw=2
"
" debugger.vim
"
" Responsible for interfacing with and communicating with the PADRE debugger
" process

let s:Running = 0
let s:PadreDebugProgram = ''
let s:JobId = 0
let s:PluginRoot = expand('<sfile>:p:h:h:h')
let s:DataItems = []
let s:NumDataWindows = 0
let s:CurrentFileLoaded = ''
let s:CurrentFileBufWindow = 0
let s:PresentDirectory = ''
let s:Debug = 0

function! padre#debugger#Setup()
  " Create buffers for PADRE
  call padre#buffer#Create('PADRE_Main', 'PADRE_Main', 0)
  call padre#buffer#Create('PADRE_Stdio', 'PADRE_Data', 1)
  " call padre#buffer#Create('PADRE_Preprocessing', 'PADRE_Preprocessing', 1)

  call padre#buffer#SetMainPadreKeyBindings('PADRE_Main')

  let s:DataItems = ['PADRE_Stdio']

  let s:PresentDirectory = expand('%:p:h')
endfunction

function! padre#debugger#Debug(...)
  call padre#layout#CloseTabsWithBuffer('PADRE_Main')
  let s:NumDataWindows = 0

  let l:program = ''
  let l:padre_host = 'localhost'
  let l:padre_port = 0
  let l:debugger = 'lldb'
  let l:debugger_type = 'lldb'

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
    if s:PadreDebugProgram != ''
      let l:program = s:PadreDebugProgram
    elseif get(g:, 'PadreDebugProgram', '') != ''
      let l:program = g:PadreDebugProgram
    else
      echoerr 'PADRE Program not found, please specify'
      return
    endif
  endif

  call padre#layout#OpenTabWithBuffer('PADRE_Main', 0)

  if l:padre_port == 0
    " call padre#debugger#AddDataWindow()
    call padre#layout#AddWindowToTab('b', 10, 'PADRE_Stdio', 0)
    wincmd b

    " TODO: Check for errors and report
    let l:command = s:PluginRoot . '/padre/target/debug/padre --debugger=' . l:debugger . ' --type=' . l:debugger_type . ' -- ' . l:program
    execute 'terminal ++curwin ' . l:command

    sleep 200ms

    let l:connection_line = ''

    for l:terminal in term_list()
      let l:connection_line = term_getline(l:terminal, 1)
      let l:match = matchlist(l:connection_line, '^Listening on \([^ ]*\):\([0-9]*\)$')
      if !empty(l:match)
        let l:padre_host = l:match[1]
        let l:padre_port = l:match[2]
      endif
    endfor
  endif

  if l:padre_port == 0
    echom "Can't connect to PADRE, unknown port"
    return
  endif

  call padre#socket#Connect(l:padre_host, l:padre_port)

  wincmd k
endfunction

function! padre#debugger#Run()
  if s:Running == 0
    echoerr 'PADRE is not running'
  endif

  call padre#socket#Send({"cmd": "run"}, function('padre#debugger#RunCallback'))
endfunction

function! padre#debugger#Stop()
  call padre#job#StopAllJobs()

  call padre#socket#Close()

  call padre#layout#CloseTabsWithBuffer('PADRE_Main')

  let s:Running = 0
endfunction

function! s:SetBreakpointInDebugger(line, file)
  call padre#socket#Send({"cmd": "breakpoint", "file": a:file, "line": str2nr(a:line)}, function('padre#debugger#BreakpointCallback'))
endfunction

function! padre#debugger#Breakpoint()
  let l:breakpointAdded = padre#signs#ToggleBreakpoint()

  if !empty(l:breakpointAdded) && s:Running == 1
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

function! padre#debugger#AddDataWindow()
  let l:created = 0
  let l:item = 0

  let l:original_winnr = winnr()

  if s:NumDataWindows == 0
    let l:pos = 'r'
  elseif s:NumDataWindows == 1 || s:NumDataWindows == 2
    wincmd l
    wincmd j
    let l:pos = 'b'
  elseif s:NumDataWindows >= 3
    echoerr 'Only 3 data windows currently supported'
  endif

  while !l:created && l:item < len(s:DataItems)
    let l:created = padre#layout#AddWindowToTab(l:pos, 40, get(s:DataItems, l:item), 0)
    let l:item += 1
  endwhile

  execute l:original_winnr . 'wincmd w'

  let s:NumDataWindows += 1
endfunction

function! padre#debugger#DataBufferFlick()
  let l:item = index(s:DataItems, padre#buffer#GetBufNameForBufNum(bufnr('%'))) + 1
  if l:item >= len(s:DataItems)
    let l:item = 0
  endif
  call padre#buffer#LoadBufferName(get(s:DataItems, l:item))
endfunction

""""""""""""""""
" API functions

function! padre#debugger#SignalPADREStarted()
  let s:Running = 1
  call padre#buffer#ClearBuffer('PADRE_Main')
  call padre#debugger#Log(4, 'PADRE debugger open')

  for l:breakpoint in padre#signs#GetAllBreakpointSigns()
    call s:SetBreakpointInDebugger(l:breakpoint['line'], l:breakpoint['file'])
  endfor
endfunction

function! padre#debugger#StdoutCallback(jobId, data, args)
  call padre#buffer#AppendBuffer('PADRE_Stdio', [a:data])
endfunction

function! padre#debugger#StderrCallback(jobId, data, args)
  call padre#buffer#AppendBuffer('PADRE_Stdio', [a:data])
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
  if a:data['status'] != 'OK'
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
    call padre#debugger#Log(2, 'Error printing variable')
    return
  endif

  let l:variable_name = remove(a:data, 'variable')

  execute "let l:json = system('python -m json.tool', '" . substitute(json_encode(a:data), "'", "''", "g") . "')"
  let l:msg = 'Variable ' . l:variable_name . '=' . l:json
  call padre#debugger#Log(4, l:msg)
endfunction

function! padre#debugger#JumpToPosition(file, line)
  let l:msg = 'Stopped file=' . a:file . ' line=' . a:line
  call padre#debugger#Log(4, l:msg)

  if a:file[0] == '/'
    let l:fileToLoad = a:file
  else
    let l:fileToLoad = s:PresentDirectory . '/' . findfile(a:file, s:PresentDirectory . '/**')
  endif

  if l:fileToLoad != s:CurrentFileLoaded
    call padre#layout#OpenTabWithBuffer('PADRE_Main', 0)

    if s:CurrentFileBufWindow == 0
      if winwidth(winnr()) <= 30
        let l:width = winwidth(winnr()) / 2
      else
        let l:width = winwidth(winnr()) - 15
      endif

      vnew
      execute 'vertical resize ' . l:height
    else
      execute s:CurrentFileBufWindow . 'wincmd w'
      call padre#buffer#UnsetPadreKeyBindings(bufname('%'))
    endif

    if filereadable(l:fileToLoad)
      execute 'view ' . l:fileToLoad

      let s:CurrentFileLoaded = l:fileToLoad
    endif

    call padre#buffer#SetMainPadreKeyBindings(s:CurrentFileLoaded)
  endif

  let s:CurrentFileBufWindow = winnr()

  call padre#signs#ReplaceCodePointer(a:line)

  execute 'normal ' . a:line . 'G'

  let s:Debug += 1
endfunction

function! padre#debugger#ProcessExited(exit_code, pid)
  call padre#debugger#Log(4, 'Process ' . a:pid . ' finished with exit code=' . a:exit_code)
endfunction

function! padre#debugger#Log(level, error_string)
  let l:log_level_set = get(g:, 'PadreLogLevel', 4)
  let l:level = ''

  if a:level > l:log_level_set
    return
  endif

  if a:level == 1
    let l:level = 'CRITICAL: '
  elseif a:level == 2
    let l:level = 'ERROR: '
  elseif a:level == 3
    let l:level = 'WARN: '
  elseif a:level == 4
    let l:level = 'INFO: '
  elseif a:level == 5
    let l:level = 'DEBUG: '
  endif

  for l:str in split(a:error_string, '\n')
    call padre#buffer#AppendBuffer('PADRE_Main', [l:level . l:str])
  endfor
endfunction
