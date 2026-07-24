import type { OperationEventDto } from "./repository";

export function updateRepositoryOperation(
  current: Readonly<Record<string, OperationEventDto>>,
  repoId: string,
  event: OperationEventDto,
): Record<string, OperationEventDto> {
  if (event.repoId !== repoId) return current;
  return { ...current, [repoId]: event };
}
