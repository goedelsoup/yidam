def union(a: list[int], b: list[int]) -> list[int]:
    return sorted(set(a) | set(b))

def intersection(a: list[int], b: list[int]) -> list[int]:
    return sorted(set(a) & set(b))

def difference(a: list[int], b: list[int]) -> list[int]:
    return sorted(set(a) - set(b))

def is_subset(a: list[int], b: list[int]) -> bool:
    return set(a) <= set(b)
