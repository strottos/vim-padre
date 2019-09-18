" vim: et ts=2 sts=2 sw=2
"
" socket.vim
"
" TODO: Socket closing and error handling

let s:connect_called = 0
let s:timer_id = 0
let s:tries = 0
let s:callback_set = 0
let s:channel_open = 0

function! padre#socket#Connect(host, port, ...)
  let s:host = a:host
  let s:port = a:port
  if a:0 > 0
    let s:callback_set = 1
    let s:callback = a:1
  endif
  call padre#socket#Close()
  let s:timer_id = timer_start(100, 'padre#socket#DoConnect')
  let s:connect_called = 1
endfunction

function! padre#socket#DoConnect(timer_id)
  let s:channel_open = 1
  let s:channel = ch_open(s:host . ':' . s:port)
  if ch_status(s:channel) is# 'fail'
    if s:tries >= 3
      let s:tries = 0
      echoerr('Cannot connect to socket')
      return
    endif
    let s:tries = s:tries + 1
    let s:timer_id = timer_start(1000, 'padre#socket#DoConnect')
  elseif ch_status(s:channel) is# 'open'
    if s:callback_set
      call s:callback()
    endif
  endif
  let s:buffer = ''
endfunction

function! padre#socket#Close()
  if s:connect_called == 1
    call timer_stop(s:timer_id)
    if ch_status(s:channel) is# 'open'
      call ch_close(s:channel)
    endif
  endif
  let s:connect_called = 0
endfunction

function! padre#socket#Status()
  if s:channel_open
    return ch_status(s:channel)
  endif
endfunction

function! padre#socket#Send(str, ...)
  if a:0 > 0
    call ch_sendexpr(s:channel, a:str, {'callback': a:1})
  endif
endfunction
