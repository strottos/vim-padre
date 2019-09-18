" vim: et ts=2 sts=2 sw=2

" This is basic vim plugin boilerplate
let s:save_cpo = &cpoptions
set cpoptions&vim

function! s:restore_cpo()
  let &cpoptions = s:save_cpo
  unlet s:save_cpo
endfunction

if exists( 'g:loaded_padre_plugin' )
  call s:restore_cpo()
  finish
elseif !has('job')
  echohl WarningMsg |
        \ echomsg 'Plugin requires vim compiled with features `job` and `timers`' |
        \ echohl None
  call s:restore_cpo()
  finish
endif

let g:loaded_padre_plugin = 1

if get(g:, 'padre_plugin_autostart', 1)
  if has( 'vim_starting' ) " Loading at startup.
    " The following technique is from the YouCompleteMe plugin.
    " We defer loading until after VimEnter to allow the gui to fork (see
    " `:h gui-fork`) and avoid a deadlock situation, as explained here:
    " https://github.com/Valloric/YouCompleteMe/pull/2473#issuecomment-267716136
    augroup vimPadrePluginStart
      autocmd!
      autocmd VimEnter * call padre#Enable()
    augroup END
  else " Manual loading with :packadd.
    call padre#Enable()
  endif
endif

command -nargs=* -complete=file PadreDebug call padre#debugger#Debug(<f-args>)
command -nargs=0 PadreRun call padre#debugger#Run()
command -nargs=0 PadreBreakpoint call padre#debugger#Breakpoint()
command -nargs=0 PadreStepIn call padre#debugger#StepIn()
command -nargs=0 PadreStepOver call padre#debugger#StepOver()
command -nargs=0 PadreStop call padre#debugger#Stop()
command -nargs=1 PadrePrintVariable call padre#debugger#PrintVariable(<f-args>)
command -nargs=0 PadreContinue call padre#debugger#Continue()

" This is basic vim plugin boilerplate
call s:restore_cpo()
