def kinetic_energy(mass: float, velocity: float) -> float:
    return 0.5 * mass * velocity * velocity

def potential_energy(mass: float, height: float, g: float) -> float:
    return mass * g * height

def power(work: float, time: float) -> float:
    return 0.0 if time == 0.0 else work / time

def efficiency(output: float, input: float) -> float:
    return 0.0 if input == 0.0 else output / input
