"""
Unit tests for VIM python code
"""

import unittest
try:
    from unittest.mock import Mock, call
except ImportError:
    # Python 2
    from mock import Mock, call

import utils


class TestUtilsMethods(unittest.TestCase):
    """
    Unit test utils.py
    """
    def test_run_vim_commands(self):
        """
        Test we can run as list as vim commands
        """
        utils.vim = Mock()
        utils.vim.command = Mock()

        utils.run_vim_commands(['test', 'test2', 'test3'])
        self.assertListEqual(
            utils.vim.command.call_args_list, [call('test'),
                                               call('test2'),
                                               call('test3')])

    def test_run_empty_vim_commands(self):
        """
        Test an empty list runs no comamnds
        """
        utils.vim = Mock()
        utils.vim.command = Mock()

        utils.run_vim_commands([])
        self.assertListEqual(utils.vim.command.call_args_list, [])

    def test_run_nonsense_vim_commands(self):
        """
        Test nonsense runs no comamnds
        """
        utils.vim = Mock()
        utils.vim.command = Mock()

        utils.run_vim_commands(None)
        self.assertListEqual(utils.vim.command.call_args_list, [])

    def test_run_single_string_vim_commands(self):
        """
        Test we can run a single string as a vim command
        """
        utils.vim = Mock()
        utils.vim.command = Mock()

        utils.run_vim_commands('test')
        self.assertListEqual(utils.vim.command.call_args_list, [call('test')])
