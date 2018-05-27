" vim: et ts=2 sts=2 sw=2
"
" preprocessing.vim

function! padre#preprocessing#CreatePreprocessingWindow()
  call padre#layout#OpenTabWithBuffer('PADRE_Main', 0)

  call padre#layout#AddWindowToTab('b', 10, 'PADRE_Preprocessing', 0)
endfunction
