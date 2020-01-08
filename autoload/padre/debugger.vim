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

  if !empty(l:breakpointAdded) && padre#socket#Status() is# "open"
    call s:SetBreakpointInDebugger(l:breakpointAdded['line'], l:breakpointAdded['file'])
  endif
endfunction

function! padre#debugger#StepIn()
  call padre#socket#Send({"cmd": "stepIn"}, function('padre#debugger#GenericCallback'))
endfunction

function! padre#debugger#StepOver()
  call padre#socket#Send({"cmd": "stepOver"}, function('padre#debugger#GenericCallback'))
endfunction

function! padre#debugger#PrintVariable(variable)
  call padre#socket#Send({"cmd": "print", "variable": a:variable}, function('padre#debugger#GenericCallback'))
endfunction

function! padre#debugger#Continue()
  call padre#socket#Send({"cmd": "continue"}, function('padre#debugger#GenericCallback'))
endfunction

" Enter the buffer that displays the threads
function! padre#debugger#ThreadsBufferEnter()
  call padre#socket#Send({"cmd": "threads"}, function('padre#debugger#GenericCallback'))
endfunction

" Activate one of the threads
function! padre#debugger#ThreadActivate()
  let l:line = getline('.')
  let l:match = matchlist(l:line, '^  | \([0-9]*\) * | .* | .*$')
  let l:thread_num = str2nr(l:match[1])
  call padre#socket#Send({"cmd": "activate_thread", "number": l:thread_num}, function('padre#debugger#GenericCallback'))
endfunction

function! padre#debugger#GetCurrentPadreNumber()
  return s:PadreNumber
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
  let l:logs_window = padre#layout#GetDataWindow()

  if l:current_window != l:logs_window
    execute l:logs_window . ' wincmd w'
  endif

  let l:current_bufnr = bufnr()
  let l:logs_bufnr = padre#layout#GetDataBufnr("Logs")

  if l:current_bufnr != l:logs_bufnr
    execute 'buffer ' . l:logs_bufnr
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

function! padre#debugger#ListThreads(threads)
  let l:current_tabpagenr = tabpagenr()

  call padre#layout#OpenTabWithBuffer('PADRE_Threads_' . s:PadreNumber)

  let l:current_window = winnr()
  let l:logs_window = padre#layout#GetDataWindow()

  if l:current_window != l:logs_window
    execute l:logs_window . ' wincmd w'
  endif

  let l:current_bufnr = bufnr()
  let l:threads_bufnr = padre#layout#GetDataBufnr("Threads")

  if l:current_bufnr != l:threads_bufnr
    execute 'buffer ' . l:threads_bufnr
  endif

  let l:width = winwidth(winnr())

  let l:number_width = 3
  let l:function_width = 10
  for l:thread in a:threads
    let l:number_width = max([l:number_width, len(l:thread["number"])])
    let l:function_width = max([l:function_width, len(l:thread["function"])])
  endfor

  if l:function_width > float2nr(l:width * 0.3)
    echom "Resetting function_width"
    let l:function_width = max([10, min([l:function_width, float2nr(l:width * 0.3)])])
    echom l:function_width
  endif

  let l:location_width = max([10, l:width - 10 - l:number_width - l:function_width])

  let l:fmt = printf("%%-s | %%-%ds | %%-%ds | %%-%ds", l:number_width, l:location_width, l:function_width)
  call padre#buffer#ClearBuffer(0)
  call padre#buffer#AppendBuffer(printf(l:fmt, " ", "num", "location", "function"), 0)
  let l:delimiter = "--+-" .  repeat("-", l:number_width) . "-+-" . repeat("-", l:location_width) . "-+-" . repeat("-", l:function_width)
  call padre#buffer#AppendBuffer(l:delimiter, 0)
  for l:thread in a:threads
    if l:thread["is_active"]
      let l:active = "*"
    else
      let l:active = " "
    endif
    let l:number = l:thread["number"]
    let l:location = l:thread["location"]
    let l:function = l:thread["function"]
    if len(l:location) > l:location_width
      let l:location = l:location[0:l:location_width - 1]
    endif
    if len(l:function) > l:function_width
      let l:function = l:function[0:l:function_width - 1]
    endif
    call padre#buffer#AppendBuffer(printf(l:fmt, l:active, l:number, l:location, l:function), 0)
  endfor

  normal 1G

  if l:current_window != l:logs_window
    execute l:current_window . ' wincmd w'
  endif

  if l:current_tabpagenr != tabpagenr()
    execute l:current_tabpagenr . 'tabnext'
  endif

  redraw
endfunction
