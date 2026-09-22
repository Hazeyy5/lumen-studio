type Handler = (data: string) => void;

const handlers = new Map<string, Handler>();
const pending = new Map<string, string>();

export const ptyBus = {
  push(sessionId: string, data: string) {
    const handler = handlers.get(sessionId);
    if (handler) {
      handler(data);
      return;
    }
    pending.set(sessionId, (pending.get(sessionId) ?? "") + data);
  },
  subscribe(sessionId: string, handler: Handler) {
    handlers.set(sessionId, handler);
    const queued = pending.get(sessionId);
    if (queued !== undefined) {
      pending.delete(sessionId);
      handler(queued);
    }
    return () => {
      if (handlers.get(sessionId) === handler) handlers.delete(sessionId);
    };
  },
};
