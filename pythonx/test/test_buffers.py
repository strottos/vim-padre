"""
VIM buffer unit tests
"""

import time
import unittest
try:
    from unittest.mock import Mock, MagicMock, call
except ImportError:
    # Python 2
    from mock import Mock, MagicMock, call

import six

import buffers
import utils


class TestBufferLists(unittest.TestCase):
    """
    Unit test buffer.BufferList
    """
    def setUp(self):
        buffers.vim = Mock()
        buffers.vim.command = Mock()
        buffers.vim.eval = Mock(side_effect=['1', '4', '5'])

        self.buffer_list = buffers.BufferList()

    def tearDown(self):
        self.buffer_list = None

    def test_create_buffer_in_list(self):
        """
        Test we can create a new buffer in a BufferList
        """
        test_buffer = self.buffer_list.create_buffer('test', [])
        self.assertEqual(test_buffer.buffer_name, 'test')
        self.assertIs(self.buffer_list.get_buffer('test'), test_buffer)

    def test_get_buffer_in_list(self):
        """
        Test we can retrieve a buffer from a BufferList
        """
        test_buffer = self.buffer_list.create_buffer('test', [])
        self.assertIs(self.buffer_list.get_buffer('test'), test_buffer)


class TestBuffers(unittest.TestCase):
    """
    Unit test creating buffers
    """
    def setUp(self):
        buffers.vim = Mock()
        buffers.vim.command = Mock()
        buffers.vim.eval = Mock(side_effect=['1', '4', '5'])
        utils.vim = Mock()
        utils.vim.command = Mock()
        utils.vim.eval = Mock(side_effect=['1', '4', '5'])

    def tearDown(self):
        buffers.vim = None

    def test_create_buffer(self):
        """
        Test we call the right vim commands to create a new buffer
        """
        test_buffer = buffers.Buffer('test', [])
        self.assertIn(call('new'), buffers.vim.command.call_args_list)
        self.assertIn(
            call('silent edit test'), buffers.vim.command.call_args_list)
        self.assertIn(call('buffer 4'), buffers.vim.command.call_args_list)
        self.assertIn(
            call('execute "1 wincmd w"'), buffers.vim.command.call_args_list)
        self.assertEqual(test_buffer.buffer_name, 'test')
        self.assertEqual(test_buffer.buffer_number, 5)

    def test_create_buffer_with_options(self):
        """
        Test we call the right vim commands to create a new buffer
        """
        buffers.Buffer('test', ['test1', 'test2', 'test3'])
        self.assertIn(call('setlocal test1'), utils.vim.command.call_args_list)
        self.assertIn(call('setlocal test2'), utils.vim.command.call_args_list)
        self.assertIn(call('setlocal test3'), utils.vim.command.call_args_list)

    def test_buffer_prepend(self):
        """
        Test we can prepend to lines in the buffer.
        """
        test_buffer = buffers.Buffer('test', [])

        self._test_write_buffer(test_buffer, 'prepend', 2, 'testing ')
        self._test_write_buffer(test_buffer, 'prepend', '4',
                                ['test line 4', 'test line 5', 'test line 6'])

    def test_buffer_append(self):
        """
        Test we can append to lines in the buffer.
        """
        test_buffer = buffers.Buffer('test', [])

        self._test_write_buffer(test_buffer, 'append', '3', ' testing')
        self._test_write_buffer(test_buffer, 'append', 5,
                                ['test line 6', 'test line 7', 'test line 8'])

    def test_buffer_replace(self):
        """
        Test we can replace lines in the buffer.
        """
        test_buffer = buffers.Buffer('test', [])

        self._test_write_buffer(test_buffer, 'replace', '5-7',
                                ['test line 6', 'test line 7', 'test line 8'])

    def test_buffer_write_with_dollar_line_num(self):
        """
        Test we can write to the buffer with the line number specified by a
        dollar sign for last line.
        """
        test_buffer = buffers.Buffer('test', [])

        self._test_write_buffer(test_buffer, 'append', '$', ' testing')

    def test_buffer_write_with_line_num_maths(self):
        """
        Test we can write to the buffer with the line number specified by a
        dollar sign for last line.
        """
        test_buffer = buffers.Buffer('test', [])

        self._test_write_buffer(test_buffer, 'prepend', '3-1', ' testing')
        self._test_write_buffer(test_buffer, 'append', '3+1', ' testing')
        self._test_write_buffer(test_buffer, 'append', '$-1', ' testing')

    def text_buffer_bad_write_throws_exception(self):
        """
        Test that when we try to write to a line beyond than the buffer
        size we throw an exception
        """
        test_buffer = buffers.Buffer('test', [])

        with self.assertRaises(buffers.BufferWriteException) as context:
            self._test_write_buffer(test_buffer, 20, 'test')

    @staticmethod
    def _test_write_buffer(buf, style, line_num, text):
        """
        Test a buffer write when we have a single range and single line

        :param buf: The buffer to write to
        :param style: 'append' or 'prepend'
        :param line_num: integer for the line to change
        :param text: text used
        """
        buffers.vim.buffers = MagicMock()
        buffers.vim.buffers.__getitem__ = Mock()
        buffers.vim.buffers.__getitem__.return_value = MagicMock()
        mock = buffers.vim.buffers.__getitem__.return_value.__setitem__ \
            = Mock()

        # Return the current line
        buffers.vim.buffers.__getitem__.return_value.__getitem__.return_value \
            = 'existing line'

        # For when dollar is specified we assume a buffer length of 10
        buffers.vim.buffers.__getitem__.return_value.__len__ = Mock()
        buffers.vim.buffers.__getitem__.return_value.__len__.return_value = 10

        if style == 'replace':
            [line_from, line_to] = line_num.split('-')
        else:
            index = str(line_num).replace('$', '10')
            index = eval(index) - 1

        if style == 'prepend':
            buf.prepend(line_num, text)

            if isinstance(text, six.string_types):
                mock.assert_called_with(index, text + 'existing line')
            else:
                mock.assert_called_with(slice(index, index), text)
        elif style == 'append':
            buf.append(line_num, text)

            if isinstance(text, six.string_types):
                mock.assert_called_with(index, 'existing line' + text)
            else:
                mock.assert_called_with(slice(index + 1, index + 1), text)
        elif style == 'replace':
            buf.replace(line_from, line_to, text)
            mock.assert_called_with(
                slice(int(line_from) - 1, int(line_to)), text)
