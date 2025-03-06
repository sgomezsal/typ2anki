import hashlib


def hash_string(input_string):
    hasher = hashlib.md5()
    hasher.update(input_string.encode('utf-8'))
    return hasher.hexdigest()