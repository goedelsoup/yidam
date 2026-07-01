import math

def modular_add(a: int, b: int, n: int) -> int:
    return (a + b) % n

def modular_mul(a: int, b: int, n: int) -> int:
    return (a * b) % n

def additive_order(a: int, n: int) -> int:
    a = a % n
    if a == 0:
        return 1
    return n // math.gcd(a, n)
