type Listener = (...args: unknown[]) => void;

export default class EventEmitter {
    private listeners = new Map<string | symbol, Set<Listener>>();

    on(event: string | symbol, listener: Listener) {
        const set = this.listeners.get(event) ?? new Set<Listener>();
        set.add(listener);
        this.listeners.set(event, set);
        return this;
    }

    off(event: string | symbol, listener: Listener) {
        const set = this.listeners.get(event);
        if (set) {
            set.delete(listener);
            if (set.size === 0) {
                this.listeners.delete(event);
            }
        }
        return this;
    }

    emit(event: string | symbol, ...args: unknown[]) {
        const set = this.listeners.get(event);
        if (!set) {
            return false;
        }
        for (const listener of set) {
            listener(...args);
        }
        return true;
    }
}
