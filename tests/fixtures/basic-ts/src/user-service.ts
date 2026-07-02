import { PolicyService } from "./policy-service";

export interface User {
  id: string;
  email: string;
}

export class UserService {
  constructor(private policies: PolicyService) {}

  create(email: string): User {
    return { id: "generated", email };
  }

  delete(id: string): void {}
}
