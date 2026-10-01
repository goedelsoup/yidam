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

## Parity tolerance

The parity tests compare with an absolute `EPSILON = 1e-4`, and its meaning depends on the
function. On `haversine_km` it is a tolerance on kilometres: 10 cm. On `bearing_deg` and
`central_angle_deg` it is a tolerance on degrees, and `1e-4°` of arc is 11 m at the
surface. The same constant is about a hundred times looser on angles than on distance.

A tolerance is needed because the functions go through `sin`, `cos` and `atan2`, which
IEEE-754 does not require to be correctly rounded. It is also absorbing a known divergence
between the implementations. TypeScript converts degrees as `x * π / 180` and Rust and Python
as `x * (π / 180)`, so `haversine_km/equator-quarter` differs by 1.8e-12 km and
`central_angle_deg/quarter-turn` by 1.4e-14°. Both fixtures hold the TypeScript value
rather than the reference's (#1280). The rule this follows is in
[Comparing floats](../parity/README.md#comparing-floats).
