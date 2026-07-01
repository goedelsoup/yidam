# geodesics domain

This domain provides pure great-circle geometry functions over the WGS-84 sphere (R = 6371.0 km). All angular inputs are in decimal degrees; distance outputs are in kilometres; bearing is in degrees clockwise from true north.

## Exposed functions

```rust
/// Great-circle distance between two points in kilometres (Haversine formula, R = 6371.0 km).
pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64;

/// Initial bearing from (lat1, lon1) to (lat2, lon2) in degrees [0, 360).
pub fn bearing_deg(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64;

/// Central angle of the great-circle arc between two points, in degrees.
pub fn central_angle_deg(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64;
```

## When to use this domain

Use this domain for lightweight spherical geometry: nearest-node lookup, bounding-box queries, or cross-language parity verification of map-distance calculations. It assumes a perfect sphere — for sub-metre geodetic accuracy use a proper WGS-84 ellipsoid library.
