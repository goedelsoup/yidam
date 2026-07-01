export function kineticEnergy(mass: number, velocity: number): number {
  return 0.5 * mass * velocity * velocity
}

export function potentialEnergy(mass: number, height: number, g: number): number {
  return mass * g * height
}

export function power(work: number, time: number): number {
  return time === 0 ? 0 : work / time
}

export function efficiency(output: number, input: number): number {
  return input === 0 ? 0 : output / input
}
