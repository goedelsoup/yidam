const R = 6371.0

export function haversineKm(lat1: number, lon1: number, lat2: number, lon2: number): number {
  const dlat = (lat2 - lat1) * Math.PI / 180
  const dlon = (lon2 - lon1) * Math.PI / 180
  const lat1r = lat1 * Math.PI / 180
  const lat2r = lat2 * Math.PI / 180
  const a = Math.sin(dlat / 2) ** 2 + Math.cos(lat1r) * Math.cos(lat2r) * Math.sin(dlon / 2) ** 2
  const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a))
  return R * c
}

export function bearingDeg(lat1: number, lon1: number, lat2: number, lon2: number): number {
  const lat1r = lat1 * Math.PI / 180
  const lat2r = lat2 * Math.PI / 180
  const dlon = (lon2 - lon1) * Math.PI / 180
  const y = Math.sin(dlon) * Math.cos(lat2r)
  const x = Math.cos(lat1r) * Math.sin(lat2r) - Math.sin(lat1r) * Math.cos(lat2r) * Math.cos(dlon)
  return (Math.atan2(y, x) * 180 / Math.PI + 360) % 360
}

export function centralAngleDeg(lat1: number, lon1: number, lat2: number, lon2: number): number {
  return haversineKm(lat1, lon1, lat2, lon2) / R * 180 / Math.PI
}
