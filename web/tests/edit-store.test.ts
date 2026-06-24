import { describe, expect, it } from 'vitest';
import { createEditStore } from '../src/edit/EditStore';

describe('EditStore', () => {
  it('records undoable delete-selected recipe', () => {
    const store = createEditStore();
    store.dispatch({ type: 'selectAll' });
    store.dispatch({ type: 'deleteSelected' });

    expect(store.recipe.operations).toEqual([{ type: 'selectAll' }, { type: 'deleteSelected' }]);
    expect(store.canUndo).toBe(true);

    store.undo();
    expect(store.recipe.operations).toEqual([{ type: 'selectAll' }]);

    store.redo();
    expect(store.recipe.operations).toEqual([{ type: 'selectAll' }, { type: 'deleteSelected' }]);
  });

  it('clears redo history after a new operation', () => {
    const store = createEditStore();
    store.dispatch({ type: 'selectAll' });
    store.undo();
    store.dispatch({ type: 'filterOpacity', min: 0.1 });

    expect(store.canRedo).toBe(false);
    expect(store.recipe.operations).toEqual([{ type: 'filterOpacity', min: 0.1 }]);
  });
});
