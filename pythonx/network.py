"""
Library for the VIM padre plugin that enables networking
"""

import socket

try:
    import vim
except ImportError:
    # Unit Testing
    pass


class Socket(object):
    """
    Sockets for interfacing with PADRE
    """
    def __init__(self):
        self._socket = socket.socket(socket.AF_INET, socket.SOCK_STREAM)

    def connect(self, port, host=None):
        """
        Create a socket and connect to a server on the host and port specified
        """
        if host == None:
            host = socket.gethostname()
        self._socket.connect((host, port))
