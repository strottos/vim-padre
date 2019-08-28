# VIM PADRE

VIM PADRE was written in order to help debug using the VIM editor. Whilst IDE's have become very popular VIM seems to remain popular (certainly in the authors case) for editing. At the time of writing this there were many debugger plugins for VIM but none that seemed to a) work, b) could debug across multiple languages and c) I could easily extend myself.

This plugin still needs a lot of work but it does work. The idea has been that we rely on an external program to provide a consistent interface for VIM with this program (that I called `padre`) that does most of the heavy lifting.

Currently `padre` supports LLDB, Python and Node debuggers with an ambition of adding more. It runs on either Linux or Mac, Windows is currently unsupported.

Here's a demo of it in action:
[![asciicast](https://asciinema.org/a/zuJTb3Nxi5uR0ObIXOCJ0TGCU.svg)](https://asciinema.org/a/zuJTb3Nxi5uR0ObIXOCJ0TGCU)

## How-To

### Installation

You can download VIM plugins in a variety of ways. The plugin is written in Rust so make sure you have Rust on your system. If you don't, you can install it via the instructions in https://rustup.rs/.

**pathogen**

`pathogen` is my favourite because you just need to add the plugin to `~/.vim/bundle`. See here for more details on using `pathogen` (https://github.com/tpope/vim-pathogen) but essentially you just clone this repository into `~/.vim/bundle`. Once you've done this you should `cd ~/.vim/bundle/vim-padre` and then run `make`. Really the `Makefile` is very simple and just handles things off to Cargo (Rust's package manager/build tool). 

**vim-plug**

Add the following to your plugin section:

```vimscript
Plug 'strottos/vim-padre', { 'dir': '~/.vim/plugged/vim-padre', 'do': 'make' }
```

### Running

Once you have VIM PADRE installed you can run a program by doing the following (NB: Do *not* specify the debugger command with the program you want to run, that's what `-d` is for):

```
:PadreDebug -d=/usr/bin/lldb -t=lldb -- ./my_prog arg1 arg2 arg3
```

You do not need to specify `-t` and `-d` and it will try and guess but if it fails to do so you can override the debugger type and debugger command respectively with these options (see Running other Debuggers below).

This will open a new tab in VIM with two open panes, one of which is the terminal command that will run the debugger and the corresponding program and the other of which is the PADRE logs. Initially you will see a log in here saying that PADRE has started, once this log comes up you may use PADRE.

### Running other Debuggers

You can specify other debuggers by using the `-t` and `-d` options. The `-t` option (or `--type`) gives us the ability to choose other debugger types, we currently support `lldb`, `node` and `python`. The `-d` (or `--debugger`) option gives us the ability to specify the path for the debugger it will use. You should not specify the debugger as part of the command you are trying to run, so for example, to run an `index.js` file through `node` you would run something like:

```
:PadreDebug -t=node -- ./index.js
```

Here you have specified that it is a debugger of type `node` but not where that debugger is, PADRE will guess that the debugger is the first thing in the PATH environment variable with name `node` in this case.

### Using PADRE

Once you have launched PADRE you can then use it. There are PADRE commands for each of the commands but it's more useful to use the keyboard shortcut. When you see the message to say PADRE is open you can type `r` in that window and it will run the program. It will try and pause the program immediately upon startup and will then (assuming it found the source code) open a new window with the source code in and a green pointer indicating whereabouts the pause occured. In either of these windows (NB: Recommend the code Window for now as there is a bug in the other window at time of writing) you can use the following commands for controlling the flow of the program:

s - Step Over (:PadreStepOver)
S - Step In (:PadreStepIn)
C - Continue (:PadreContinue)

You can print variables by visually highlighting them and pressing `p`. You can also set breakpoints by going to the appropriate file and doing either `:PadreBreakpoint` or by adding the following to your `.vimrc` and then simply doing `-b` where you want the breakpoint:

```
let g:maplocalleader='-'
nnoremap <localleader>b :PadreBreakpoint<cr>
```

You can of course choose your own letter for doing this but I recommend using a local leader. See here for more details about leaders and local leaders in VIM: http://learnvimscriptthehardway.stevelosh.com/chapters/06.html

Note currently unsetting breakpoints is not supported, this should be added at some point.

You can also interface with the terminal, anything you type in will be forwarded to PADRE and then quite often from there to the Debugger itself (and often onto the program itself). 

## Layout and Architecture

VIM itself is intended to be as dumb as possible when using PADRE and it simply has a command that spawns a PADRE process and others that simple send responses and listen for simple instructions.

The PADRE process itself is more interesting and more complex. PADRE originally started in Go but that didn't work out so well and I wanted a better and faster prototype so I switched to using Node. Then Node never gave me the power I wanted and frankly I wanted to learn Rust, so this got rewritten in Rust and that's what I'm using now. It uses Tokio to be able to support multiple things happening at once.

You can open a separate connection to PADRE in order to debug it, or run a separate PADRE command on the command line and tell VIM to connect into that.

## Issues

There is still a lot of work to be done on this plugin, the debugger since being written in Rust is better now but still some extra error handling wouldn't go amiss, particularly in Node and Python. The VIM interface, however, still needs a lot of work itself, that has quite a few bugs in.

### TODOs

Things that we need to add still are as follows (feel free to help if you wish, some are likely to be more challenging than others):
- Support requesting non-existent files, e.g. assembly for LLDB and internal scripts for Node.
- Queueing and counting of requests, would be nice to be able to do 3s and it steps over 3 times but without sending 3 commands indicating where it is.
- Configurably auto step ins till we find code
- Remove breakpoints
- Interrupts
- Support for multi-threading/multi-processing
- Backtraces
- Add in preprocessing possibilities like compiling before running PADRE
- Profiling CPU, mem, etc
- Proper variable printing, it's mostly a bit simple at the moment
- Go Debugger
- Java Debugger
- Padre can be ran multiple times without restarting vim (Currently I restart VIM every time I want to run PADRE, this is a serious bug that needs fixing ASAP)
- Support multiple PADRE processes
- Make things more configurable
- Consistent breakpoint setting, can set them before or after running the program and they will still be picked up, works better unders some debuggers than others
- Upgrade PADRE to use new Tokio/Rust futures/async/await syntax once they're in stable.
