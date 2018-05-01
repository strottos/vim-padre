"""
Library for the VIM padre plugin that interfaces with VIM buffers
"""

import six

import utils

try:
    import vim
except ImportError:
    # Unit Testing
    pass


class BufferWriteException(Exception):
    """
    Buffer Writing Exception class
    """
    pass


class BufferList(object):
    """
    VIM Buffer Lists
    """
    def __init__(self):
        self._buffers = []

    def create_buffer(self, name, commands):
        """
        Check if a buffer exists and create it if not.

        Returns: The Buffer object
        """
        buffer_object = Buffer(name, commands)
        self._buffers.append(buffer_object)

        return buffer_object

    def get_buffer(self, name):
        """
        Return the buffer with the specified name
        """
        for buffer_object in self._buffers:
            if (type(name) is str and buffer_object.buffer_name == name) or \
                    (type(name) is int and buffer_object.buffer_number == name):
                return buffer_object

        return None


class Buffer(object):
    """
    VIM Buffer Objects
    """
    def __init__(self, name, commands):
        """
        Constructor that will create a vim buffer
        """
        self._buffer_number = None
        self._buffer_name = None
        self._create(name, commands)

    def _create(self, name, commands):
        """
        Creates a vim buffer out of the name and run the commands supplied.
        """
        current_window_number = vim.eval("winnr()")
        current_buffer_number = vim.eval("bufnr('%')")
        vim.command("new")
        vim.command("silent edit " + name)
        self._buffer_number = int(vim.eval("bufnr('%')"))
        self._buffer_name = name
        utils.run_vim_commands(commands)
        vim.command("quit")

        vim.command("execute '" + current_window_number + " wincmd w'")
        vim.command("buffer " + current_buffer_number)

    @property
    def buffer_name(self):
        """
        Return the buffers name
        """
        return self._buffer_name

    @property
    def buffer_number(self):
        """
        Return the buffers vim buffer number
        """
        return self._buffer_number

    def read(self):
        """
        Return the contents of the buffer
        """
        return vim.buffers[self.buffer_number][:]

    def prepend(self, line_num, text):
        """
        Given a line number and text we write them to the buffer

        :param line: The line is either an integer, a `$` or
            addition/subtraction of these two. This indicates the line to
            prepend to. In the case we have a `$` this indicates the last line.
        :param text: A string or a list of strings. If it's a string that will
            be prepended to the line specified and if it's a list of strings
            each line will be added above or below the line specified.
        """
        edit_buffer = vim.buffers[self.buffer_number]
        line_num = self._expand_line(line_num)
        existing_line = edit_buffer[int(line_num) - 1]

        if isinstance(text, six.string_types):
            text = text + existing_line
            self._do_write_buffer(line_num, line_num, text,
                                  self._edit_function_replace_lines)
        else:
            self._do_write_buffer(line_num, line_num, text,
                                  self._edit_function_prepend_lines)

    def append(self, line_num, text):
        """
        Given a line number and text we write them to the buffer

        :param line: The line is either an integer, a `$` or
            addition/subtraction of these two. This indicates the line to
            append to. In the case we have a `$` this indicates the last line.
        :param text: A string or a list of strings. If it's a string that will
            be appended to the line specified and if it's a list of strings
            each line will be added above or below the line specified.
        """
        edit_buffer = vim.buffers[self.buffer_number]
        line_num = self._expand_line(line_num)
        existing_line = edit_buffer[line_num - 1]

        if isinstance(text, six.string_types):
            text = existing_line + text
            self._do_write_buffer(line_num, line_num, text,
                                  self._edit_function_replace_lines)
        else:
            self._do_write_buffer(line_num, line_num, text,
                                  self._edit_function_append_lines)

    def replace(self, line_from, line_to, text):
        """
        Given a line number and text we write them to the buffer

        :param line_from: Either an integer, a `$` or addition/subtraction of
            these two. This indicates the line to append to. In the case we
            have a `$` this indicates the last line.
        :param line_to: Either an integer, a `$` or addition/subtraction of
            these two. This indicates the line to append to. In the case we
            have a `$` this indicates the last line.
        :param text: A string or a list of strings. If it's a string that will
            be appended to the line specified and if it's a list of strings
            each line will be added above or below the line specified.
        """
        line_from = self._expand_line(line_from)
        line_to = self._expand_line(line_to)
        self._do_write_buffer(line_from, line_to + 1, text,
                              self._edit_function_replace_lines)

    def _expand_line(self, line_num):
        """
        Expand the line_string parameter to the correct line number

        :param line_string: A string containing either "%", "$" or an integer
        :return: The integer of the line_string given
        """
        edit_buffer = vim.buffers[self.buffer_number]
        line_num = str(line_num).replace("$", str(len(edit_buffer)))
        line_num = eval(line_num)

        if line_num > len(edit_buffer):
            raise BufferWriteException()

        return line_num

    def _do_write_buffer(self, line_from, line_to, text, edit_fn):
        """
        Actually do a simple buffer write
        """
        edit_buffer = vim.buffers[self.buffer_number]
        was_modifiable = edit_buffer.options["modifiable"]
        if not was_modifiable:
            edit_buffer.options["modifiable"] = True

        edit_fn(line_from, line_to, text)

        if not was_modifiable:
            edit_buffer.options["modifiable"] = False

    def _edit_function_replace_lines(self, line_from, line_to, text):
        """
        Replace line_from to line_to with text
        """
        edit_buffer = vim.buffers[self.buffer_number]
        if line_from == line_to:
            edit_buffer[line_from - 1] = text
        else:
            edit_buffer[line_from - 1:line_to - 1] = text

    def _edit_function_prepend_lines(self, line_from, line_to, text):
        """
        Prepend line_from (== line_to) with text
        """
        if line_from != line_to:
            raise BufferWriteException
        edit_buffer = vim.buffers[self.buffer_number]
        edit_buffer[line_from - 1:line_from - 1] = text

    def _edit_function_append_lines(self, line_from, line_to, text):
        """
        Append line_from (== line_to) with text
        """
        if line_from != line_to:
            raise BufferWriteException
        edit_buffer = vim.buffers[self.buffer_number]
        edit_buffer[line_to:line_to] = text
