export type SelectMode = 'set' | 'add' | 'remove';

export type EditOperation =
  | { type: 'selectAll' }
  | { type: 'selectNone' }
  | { type: 'selectBox'; mode: SelectMode; center: [number, number, number]; size: [number, number, number] }
  | { type: 'deleteSelected' }
  | { type: 'transformSelected'; matrix: number[] }
  | { type: 'filterOpacity'; min: number };

export type EditRecipe = { version: 1; operations: EditOperation[] };

export type EditStore = {
  readonly recipe: EditRecipe;
  readonly canUndo: boolean;
  readonly canRedo: boolean;
  dispatch(operation: EditOperation): void;
  undo(): void;
  redo(): void;
  reset(): void;
  subscribe(listener: () => void): () => void;
};

function identityMatrix(): number[] {
  return [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
}

export function translateOperation(x: number, y: number, z: number): EditOperation {
  const matrix = identityMatrix();
  matrix[12] = x;
  matrix[13] = y;
  matrix[14] = z;
  return { type: 'transformSelected', matrix };
}

export function createEditStore(initial: EditRecipe = { version: 1, operations: [] }): EditStore {
  let operations = [...initial.operations];
  let undone: EditOperation[] = [];
  const listeners = new Set<() => void>();

  const notify = () => {
    for (const listener of listeners) listener();
  };

  return {
    get recipe() {
      return { version: 1, operations: [...operations] };
    },
    get canUndo() {
      return operations.length > 0;
    },
    get canRedo() {
      return undone.length > 0;
    },
    dispatch(operation) {
      operations = [...operations, operation];
      undone = [];
      notify();
    },
    undo() {
      const operation = operations.at(-1);
      if (!operation) return;
      operations = operations.slice(0, -1);
      undone = [operation, ...undone];
      notify();
    },
    redo() {
      const operation = undone[0];
      if (!operation) return;
      operations = [...operations, operation];
      undone = undone.slice(1);
      notify();
    },
    reset() {
      operations = [];
      undone = [];
      notify();
    },
    subscribe(listener) {
      listeners.add(listener);
      return () => listeners.delete(listener);
    },
  };
}
