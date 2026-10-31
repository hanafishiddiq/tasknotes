type Handler = (event: { event: string; id: number; payload: unknown }) => unknown;

/** Stands in for events the desktop shell sends to the page. */
export const tauriEvents = {
  handlers: new Map<string, Set<Handler>>(),

  listen(name: string, handler: Handler): () => void {
    const set = this.handlers.get(name) ?? new Set<Handler>();
    set.add(handler);
    this.handlers.set(name, set);
    return () => set.delete(handler);
  },

  /** Sends an event and waits until every listener has finished with it. */
  async emit(name: string, payload: unknown = null): Promise<void> {
    for (const handler of [...(this.handlers.get(name) ?? [])]) {
      await handler({ event: name, id: 0, payload });
    }
  },
};
