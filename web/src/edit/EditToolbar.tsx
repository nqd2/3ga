import { MousePointer2, Move3D, Redo2, SlidersHorizontal, Trash2, Undo2, X } from 'lucide-react';
import type { ReactNode } from 'react';
import type { EditOperation, EditStore } from './EditStore';
import { translateOperation } from './EditStore';

function ToolButton({
  label,
  disabled,
  onClick,
  children,
}: {
  label: string;
  disabled?: boolean;
  onClick: () => void;
  children: ReactNode;
}) {
  return (
    <button className="tool-button" type="button" aria-label={label} title={label} disabled={disabled} onClick={onClick}>
      {children}
    </button>
  );
}

export function EditToolbar({ store, onChange }: { store: EditStore; onChange: () => void }) {
  const dispatch = (operation: EditOperation) => {
    store.dispatch(operation);
    onChange();
  };

  return (
    <div className="toolbar" aria-label="Edit tools">
      <ToolButton label="Select all" onClick={() => dispatch({ type: 'selectAll' })}>
        <MousePointer2 size={18} />
      </ToolButton>
      <ToolButton
        label="Box select"
        onClick={() =>
          dispatch({ type: 'selectBox', mode: 'set', center: [0, 0, 0], size: [1, 1, 1] })
        }
      >
        <span className="box-icon" />
      </ToolButton>
      <ToolButton label="Clear selection" onClick={() => dispatch({ type: 'selectNone' })}>
        <X size={18} />
      </ToolButton>
      <ToolButton label="Move selection" onClick={() => dispatch(translateOperation(0.1, 0, 0))}>
        <Move3D size={18} />
      </ToolButton>
      <ToolButton label="Opacity filter" onClick={() => dispatch({ type: 'filterOpacity', min: 0.08 })}>
        <SlidersHorizontal size={18} />
      </ToolButton>
      <ToolButton label="Delete selected" onClick={() => dispatch({ type: 'deleteSelected' })}>
        <Trash2 size={18} />
      </ToolButton>
      <span className="toolbar__split" />
      <ToolButton
        label="Undo"
        disabled={!store.canUndo}
        onClick={() => {
          store.undo();
          onChange();
        }}
      >
        <Undo2 size={18} />
      </ToolButton>
      <ToolButton
        label="Redo"
        disabled={!store.canRedo}
        onClick={() => {
          store.redo();
          onChange();
        }}
      >
        <Redo2 size={18} />
      </ToolButton>
    </div>
  );
}
