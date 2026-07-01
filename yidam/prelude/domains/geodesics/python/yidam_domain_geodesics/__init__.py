import math

_R = 6371.0

def haversine_km(lat1: float, lon1: float, lat2: float, lon2: float) -> float:
    dlat = math.radians(lat2 - lat1)
    dlon = math.radians(lon2 - lon1)
    lat1r = math.radians(lat1)
    lat2r = math.radians(lat2)
    a = math.sin(dlat / 2) ** 2 + math.cos(lat1r) * math.cos(lat2r) * math.sin(dlon / 2) ** 2
    c = 2 * math.atan2(math.sqrt(a), math.sqrt(1 - a))
    return _R * c

def bearing_deg(lat1: float, lon1: float, lat2: float, lon2: float) -> float:
    lat1r = math.radians(lat1)
    lat2r = math.radians(lat2)
    dlon = math.radians(lon2 - lon1)
    y = math.sin(dlon) * math.cos(lat2r)
    x = math.cos(lat1r) * math.sin(lat2r) - math.sin(lat1r) * math.cos(lat2r) * math.cos(dlon)
    return (math.degrees(math.atan2(y, x)) + 360) % 360

def central_angle_deg(lat1: float, lon1: float, lat2: float, lon2: float) -> float:
    return haversine_km(lat1, lon1, lat2, lon2) / _R * 180 / math.pi
