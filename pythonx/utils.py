"""
Python utils for the VIM padre plugin
"""

import socket
import six

try:
    import vim
except ImportError:
    # Unit Testing
    pass


def get_unused_localhost_port():
    """
    Find an unused port. Based on a similar function in YouCompleteMe.
    """
    sock = socket.socket()
    # This tells the OS to give us any free port in the range [1024 - 65535]
    sock.bind(('', 0))
    port = sock.getsockname()[1]
    sock.close()
    return port


def run_vim_commands(commands):
    """
    Given a list or single commands we run them one by one
    """
    if isinstance(commands, six.string_types):
        commands = [commands]

    if not isinstance(commands, list):
        return

    for command in commands:
        vim.command(command)
