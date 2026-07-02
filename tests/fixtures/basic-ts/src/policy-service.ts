export function calculatePremium(age: number): number {
  return age * 12;
}

export class PolicyService {
  quote(age: number): number {
    return calculatePremium(age);
  }
}
