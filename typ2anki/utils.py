import hashlib
from typing import List


def hash_string(input_string):
    hasher = hashlib.md5()
    hasher.update(input_string.encode('utf-8'))
    return hasher.hexdigest()

def print_header(lines: List[str],width: int = 0,border_char='='):
    """
    Print a header with a border and centered text.
    
    Args:
        lines (List[str]): The lines to print in the header.
        width (int): The width of the header. Default is 0, so it will take max(<max line length> + 10,80).
        border_char (str): The character to use for the border. Default is '='.
    """
    if width == 0:
        width = max(max(len(line) for line in lines) + 10, 80)
    if width < 0:
        raise ValueError("Width must be a positive integer.")
    
    print(border_char * width)
    for line in lines:
        print(line.center(width))
    print(border_char * width)