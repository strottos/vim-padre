" vim: et ts=2 sts=2 sw=2
"
" socket.vim
"
" TODO: Socket closing and error handling

let s:connect_called = 0
let s:timer_id = 0
let s:tries = 0

function! padre#socket#Connect(host, port)
  let s:host = a:host
  let s:port = a:port
  call padre#socket#Close()
  let s:timer_id = timer_start(100, 'padre#socket#DoConnect')
  let s:connect_called = 1
endfunction

function! padre#socket#DoConnect(timer_id)
  let s:channel = ch_open(s:host . ':' . s:port)
  if ch_status(s:channel) is# 'fail'
    if s:tries >= 3
      let s:tries = 0
      echoerr('Cannot connect to socket')
      return
    endif
    let s:tries = s:tries + 1
    let s:timer_id = timer_start(1000, 'padre#socket#DoConnect')
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
  return ch_status(s:channel)
endfunction

function! padre#socket#Send(str, ...)
  if a:0 > 0
    call ch_sendexpr(s:channel, a:str, {'callback': a:1})
  else
    call ch_sendexpr(s:channel, a:str, {'callback': 'padre#socket#Receive'})
  endif
endfunction

function! padre#socket#Receive(channel, msg)
  let s:buffer = s:buffer . a:msg
endfunction

function! padre#socket#Received()
  return s:buffer
endfunction
