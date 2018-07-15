"""
API for the VIM PADRE plugin

Most functions in this file should be kept very short and are used
only to let VIM interface with the Python code.
"""

import vim

import utils


class BufferNotFoundException(Exception):
    """
    Buffer not found exception class
    """
    pass


class API(object):
    """
    VIM Plugin API
    """
    def __init__(self):
        self._server_popen = None

    @staticmethod
    def get_unused_localhost_port():
        """
        Get a free port on localhost to run padre
        """
        return utils.get_unused_localhost_port()
