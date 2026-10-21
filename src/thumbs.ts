import { inTauri, readThumbnail } from "./api";

const ready = new Map<string, string>();
const pending = new Map<string, Promise<string | null>>();
const queue: Array<() => void> = [];
let running = 0;

function pump() {
  while (running < 3 && queue.length > 0) {
    running += 1;
    const job = queue.shift();
    job?.();
  }
}

export function loadThumbnail(path: string): Promise<string | null> {
  if (!inTauri()) return Promise.resolve(null);
  const cached = ready.get(path);
  if (cached) return Promise.resolve(cached);
  const existing = pending.get(path);
  if (existing) return existing;
  const promise = new Promise<string | null>((resolve) => {
    queue.push(() => {
      void (async () => {
        try {
          const image = await readThumbnail(path);
          const url = `data:${image.mime};base64,${image.base64}`;
          ready.set(path, url);
          resolve(url);
        } catch {
          resolve(null);
        } finally {
          running -= 1;
          pending.delete(path);
          pump();
        }
      })();
    });
  });
  pending.set(path, promise);
  pump();
  return promise;
}
