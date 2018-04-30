"""
Networking unit tests
"""

import socket

import unittest
try:
    from unittest.mock import Mock, MagicMock, call
except ImportError:
    # Python 2
    from mock import Mock, MagicMock, call

import network


class TestSocket(unittest.TestCase):
    """
    Unit test network.Socket
    """
    def setUp(self):
        network.socket.socket = Mock()
        network.socket.socket.return_value = Mock()
        network.socket.socket.return_value.connect = Mock()
        network.socket.gethostname = Mock()
        network.socket.gethostname.return_value = 'localhost'
        self.socket = network.Socket()

    def tearDown(self):
        pass

    def test_create_socket_client(self):
        """
        Test we can create a socket that connects to a server
        """
        self.socket.connect(12345)
        self.assertIn(call(socket.AF_INET, socket.SOCK_STREAM),
                      network.socket.socket.call_args_list)
        self.assertIn(call(), network.socket.gethostname.call_args_list)
        self.assertIn(call(('localhost', 12345)),
                      network.socket.socket.return_value.connect.call_args_list)
