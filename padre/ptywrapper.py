#!/usr/bin/env python

from select import select
import datetime
import os
import pty
import signal
import sys
import tty

STDIN_FILENO = 0
STDOUT_FILENO = 1
STDERR_FILENO = 2

CHILD = 0
PID = -1
TTY_MODE = None


def receiveSignal(signalNumber, frame):
    fid = open("/tmp/ptyout_killing", "w")
    fid.write("Signal {}".format(signalNumber))
    fid.close()

    if PID != -1:
        os.kill(PID, signalNumber)

    if TTY_MODE:
        tty.tcsetattr(STDIN_FILENO, tty.TCSAFLUSH, TTY_MODE)

    sys.exit(0)


def _writen(fd, data):
    """Write all the data to a descriptor."""
    while data:
        n = os.write(fd, data)
        data = data[n:]


def _read(fd):
    """Default read function."""
    return os.read(fd, 1024)


def _copy(master_fd, master_read=_read, stdin_read=_read):
    """Parent copy loop.
    Copies
            pty master -> standard output   (master_read)
            standard input -> pty master    (stdin_read)"""
    fds = [master_fd, STDIN_FILENO]
    while True:
        rfds, wfds, xfds = select(fds, [], [])
        if master_fd in rfds:
            data = master_read(master_fd)
            if not data:  # Reached EOF.
                fds.remove(master_fd)
            else:
                os.write(STDOUT_FILENO, data)
        if STDIN_FILENO in rfds:
            data = stdin_read(STDIN_FILENO)
            if not data:
                fds.remove(STDIN_FILENO)
            else:
                _writen(master_fd, data)


def spawn(argv, master_read=_read, stdin_read=_read):
    """Create a spawned process."""
    global PID, TTY_MODE

    PID, master_fd = pty.fork()

    if PID == CHILD:
        os.execlp(argv[0], *argv)

    # Master
    signal.signal(signal.SIGHUP, receiveSignal)
    signal.signal(signal.SIGINT, receiveSignal)
    signal.signal(signal.SIGQUIT, receiveSignal)
    signal.signal(signal.SIGILL, receiveSignal)
    signal.signal(signal.SIGTRAP, receiveSignal)
    signal.signal(signal.SIGABRT, receiveSignal)
    signal.signal(signal.SIGBUS, receiveSignal)
    signal.signal(signal.SIGFPE, receiveSignal)
    #signal.signal(signal.SIGKILL, receiveSignal)
    signal.signal(signal.SIGUSR1, receiveSignal)
    signal.signal(signal.SIGSEGV, receiveSignal)
    signal.signal(signal.SIGUSR2, receiveSignal)
    signal.signal(signal.SIGPIPE, receiveSignal)
    signal.signal(signal.SIGALRM, receiveSignal)
    signal.signal(signal.SIGTERM, receiveSignal)
    try:
        mode = tty.tcgetattr(STDIN_FILENO)
        tty.setraw(STDIN_FILENO)
        TTY_MODE = mode
    except tty.error:    # This is the same as termios.error
        pass

    try:
        _copy(master_fd, master_read, stdin_read)
    except OSError:
        pass

    if TTY_MODE:
        tty.tcsetattr(STDIN_FILENO, tty.TCSAFLUSH, TTY_MODE)

    os.close(master_fd)
    return os.waitpid(PID, 0)[1]


spawn(sys.argv[1:])
