#!/usr/bin/env python3
"""Write the uploader password.conf verified by retrodeck-native.

Reads one password line from stdin and writes the PBKDF2-HMAC-SHA256
configuration to the path given as the only argument.
"""

import base64
import hashlib
import os
import sys

ITERATIONS = 210000


def main() -> int:
    if len(sys.argv) != 2:
        print("usage: set-uploader-password.py PASSWORD.CONF", file=sys.stderr)
        return 2
    line = sys.stdin.buffer.readline(130)
    password = line.rstrip(b"\n").rstrip(b"\r")
    if not 8 <= len(password) <= 128:
        print("password must contain 8 through 128 bytes", file=sys.stderr)
        return 1
    if b"\r" in password or b"\n" in password or b"\x00" in password:
        print("password contains forbidden control characters", file=sys.stderr)
        return 1
    salt = os.urandom(16)
    digest = hashlib.pbkdf2_hmac("sha256", password, salt, ITERATIONS)
    encode = lambda value: base64.b64encode(value).rstrip(b"=").decode()
    text = "version=1\niterations={}\nsalt={}\ndigest={}\n".format(
        ITERATIONS, encode(salt), encode(digest)
    )
    descriptor = os.open(
        sys.argv[1], os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600
    )
    with os.fdopen(descriptor, "w") as output:
        output.write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
