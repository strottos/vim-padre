"""
API for the VIM PADRE plugin

Most functions in this file should be kept very short and are used
only to let VIM interface with the Python code.
"""
import subprocess

import vim
import time

import buffers
import network
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
        self._buffers_list = buffers.BufferList()

#    def run_padre(self):
#        """
#        Spawn padre command
#        """
#        server_port = utils.get_unused_localhost_port()
#        args = ['padre', '--port={0}'.format(server_port)]
#
#        self._server_popen = subprocess.Popen(args,
#                                              stdout=subprocess.PIPE,
#                                              stderr=subprocess.PIPE)
#        return server_port

    def create_buffer(self, name, options):
        """
        API call to created a buffer

        Returns the buffer number
        """
        return self._buffers_list.create_buffer(name, options).buffer_number

    def get_buffer(self, name):
        """
        API call to return a vim buffer

        Returns None if the buffer doesn't exist otherwise returns the
        buffer number
        """
        return self._buffers_list.get_buffer(name).buffer_number

    def prepend_buffer(self, name, line, text):
        """
        API call to write to the buffer specified
        """
        buf = self._buffers_list.get_buffer(name)
        if buf is None:
            raise BufferNotFoundException(name)
        buf.prepend(line, text)

    def append_buffer(self, name, line, text):
        """
        API call to write to the buffer specified
        """
        buf = self._buffers_list.get_buffer(name)
        if buf is None:
            raise BufferNotFoundException(name)
        buf.append(line, text)

    def replace_buffer(self, name, line_from, line_to, text):
        """
        API call to write to the buffer specified
        """
        buf = self._buffers_list.get_buffer(name)
        if buf is None:
            raise BufferNotFoundException(name)
        buf.replace(line_from, line_to, text)

    def clear_buffer(self, name):
        """
        Empty the buffer specified.
        """
        buf = self._buffers_list.get_buffer(name)
        if buf is None:
            raise BufferNotFoundException(name)
        buf.replace(1, '$', None)

    def get_unused_localhost_port(self):
        """
        Get a free port on localhost to run padre
        """
        return utils.get_unused_localhost_port()

    @staticmethod
    def parse_output(text, param):
        """
        TODO
        """
        pass
