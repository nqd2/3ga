export type FlyAxes = {
  forward: number;
  right: number;
  up: number;
  fast: boolean;
  slow: boolean;
};

export class KeyboardFlyInput {
  private pressed = new Set<string>();

  constructor() {
    window.addEventListener('keydown', this.handleKeyDown);
    window.addEventListener('keyup', this.handleKeyUp);
    window.addEventListener('blur', this.clear);
  }

  destroy() {
    window.removeEventListener('keydown', this.handleKeyDown);
    window.removeEventListener('keyup', this.handleKeyUp);
    window.removeEventListener('blur', this.clear);
  }

  axes(): FlyAxes {
    return {
      forward: axis(this.pressed, 'KeyW', 'KeyS'),
      right: axis(this.pressed, 'KeyD', 'KeyA'),
      up: axis(this.pressed, 'KeyE', 'KeyQ'),
      fast: this.pressed.has('ShiftLeft') || this.pressed.has('ShiftRight'),
      slow: this.pressed.has('AltLeft') || this.pressed.has('AltRight'),
    };
  }

  private handleKeyDown = (event: KeyboardEvent) => {
    if (isEditableTarget(event.target)) return;
    if (trackedKeys.has(event.code)) {
      this.pressed.add(event.code);
      event.preventDefault();
    }
  };

  private handleKeyUp = (event: KeyboardEvent) => {
    this.pressed.delete(event.code);
  };

  private clear = () => {
    this.pressed.clear();
  };
}

const trackedKeys = new Set([
  'KeyW',
  'KeyA',
  'KeyS',
  'KeyD',
  'KeyQ',
  'KeyE',
  'ShiftLeft',
  'ShiftRight',
  'AltLeft',
  'AltRight',
]);

function axis(pressed: Set<string>, positive: string, negative: string) {
  return (pressed.has(positive) ? 1 : 0) - (pressed.has(negative) ? 1 : 0);
}

function isEditableTarget(target: EventTarget | null) {
  if (!(target instanceof HTMLElement)) return false;
  const tag = target.tagName.toLowerCase();
  return tag === 'input' || tag === 'textarea' || tag === 'select' || target.isContentEditable;
}
