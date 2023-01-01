#!/usr/bin/env python3
import sys

s = input()

if ord(s[-1]) - ord(s[-2]) > 40:
    sys.exit(20)
